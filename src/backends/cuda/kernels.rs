use super::CudaRuntime;
use crate::bus::{
    gravity_interval_ns, Cell, InputPins, PieceType, SystemBus, BOARD_COLS, BOARD_ROWS,
};
use crate::chips::tetrominoes::{I_KICKS, JLSTZ_KICKS, TETROMINOES};

use rustacuda::launch;
use rustacuda::memory::{CopyDestination, DeviceBuffer};

impl CudaRuntime {
    pub(super) fn run_collision_chip(&mut self, bus: &mut SystemBus) -> Result<(), String> {
        self.upload_board(bus)?;
        self.upload_piece_cells(bus.piece_type.0, bus.piece_rotation)?;

        bus.wires.collision_any = self.piece_collides(bus.piece_x, bus.piece_y)?;
        bus.wires.collision_down = self.piece_collides(bus.piece_x, bus.piece_y + 1)?;
        Ok(())
    }

    pub(super) fn run_input_decoder_chip(
        &mut self,
        pins: &InputPins,
        bus: &mut SystemBus,
    ) -> Result<(), String> {
        let _ = &self.module;

        if pins.key_down {
            bus.wires.dy = 1;
        }

        let up_edge = pins.key_up && !bus.prev_key_up;
        let x_edge = pins.key_x && !bus.prev_key_x;
        let z_edge = pins.key_z && !bus.prev_key_z;
        let space_edge = pins.key_space && !bus.prev_key_space;
        let c_edge = pins.key_c && !bus.prev_key_c;
        let enter_edge = pins.key_enter && !bus.prev_key_enter;

        bus.wires.rotate_cw = up_edge || x_edge;
        bus.wires.rotate_ccw = z_edge;
        bus.wires.hard_drop_requested = space_edge;
        bus.wires.hold_requested = c_edge;
        bus.wires.pause_requested = enter_edge;
        Ok(())
    }

    pub(super) fn run_gravity_timer_chip(
        &mut self,
        pins: &InputPins,
        bus: &mut SystemBus,
    ) -> Result<(), String> {
        let _ = &self.stream;

        bus.gravity_accumulator_ns = bus
            .gravity_accumulator_ns
            .saturating_add(pins.frame_delta_ns);

        while bus.gravity_accumulator_ns >= bus.gravity_interval_ns {
            bus.wires.gravity_tick = true;
            bus.wires.dy = 1;
            bus.gravity_accumulator_ns -= bus.gravity_interval_ns;
        }
        Ok(())
    }

    pub(super) fn run_das_timer_chip(
        &mut self,
        pins: &InputPins,
        bus: &mut SystemBus,
    ) -> Result<(), String> {
        let left = pins.key_left;
        let right = pins.key_right;
        let prev_left = bus.prev_key_left;
        let prev_right = bus.prev_key_right;

        let left_just_pressed = left && !prev_left;
        let right_just_pressed = right && !prev_right;
        let any_released = (!left && prev_left) || (!right && prev_right);

        if left_just_pressed && !right {
            bus.wires.das_tick = true;
            bus.wires.dx = -1;
            bus.das_accumulator_ns = 0;
            bus.das_active = true;
            bus.das_direction = -1;
            bus.das_last_repeat_index = 0;
        } else if right_just_pressed && !left {
            bus.wires.das_tick = true;
            bus.wires.dx = 1;
            bus.das_accumulator_ns = 0;
            bus.das_active = true;
            bus.das_direction = 1;
            bus.das_last_repeat_index = 0;
        } else if left_just_pressed && right {
            bus.wires.das_tick = true;
            bus.wires.dx = -1;
            bus.das_accumulator_ns = 0;
            bus.das_active = true;
            bus.das_direction = -1;
            bus.das_last_repeat_index = 0;
        } else if right_just_pressed && left {
            bus.wires.das_tick = true;
            bus.wires.dx = 1;
            bus.das_accumulator_ns = 0;
            bus.das_active = true;
            bus.das_direction = 1;
            bus.das_last_repeat_index = 0;
        } else if any_released {
            bus.das_active = false;
            bus.das_direction = 0;
            bus.wires.dx = 0;
            bus.das_accumulator_ns = 0;
            bus.das_last_repeat_index = 0;
        } else if bus.das_active && bus.das_direction != 0 {
            bus.das_accumulator_ns = bus.das_accumulator_ns.saturating_add(pins.frame_delta_ns);

            let acc = bus.das_accumulator_ns;
            if acc >= bus.das_delay_ns {
                let elapsed = acc - bus.das_delay_ns;
                let repeat_idx = (elapsed / bus.das_repeat_ns) as u32;
                if repeat_idx > bus.das_last_repeat_index {
                    bus.wires.das_tick = true;
                    bus.wires.dx = bus.das_direction;
                    bus.das_last_repeat_index = repeat_idx;
                }
            }
        }

        Ok(())
    }

    pub(super) fn run_rotation_chip(&mut self, bus: &mut SystemBus) -> Result<(), String> {
        if !bus.wires.rotate_cw && !bus.wires.rotate_ccw {
            return Ok(());
        }

        let from_rot = bus.piece_rotation as usize;
        let to_rot = if bus.wires.rotate_cw {
            (from_rot + 1) & 3
        } else {
            (from_rot + 3) & 3
        };

        let pt = bus.piece_type.0 as usize;

        self.upload_board(bus)?;
        self.upload_piece_cells(bus.piece_type.0, to_rot as u8)?;

        if pt == 3 {
            if !self.piece_collides(bus.piece_x, bus.piece_y)? {
                bus.piece_rotation = to_rot as u8;
                bus.wires.collision_any = false;
                bus.wires.render_dirty = true;
            }
            return Ok(());
        }

        let kicks = if pt == 0 {
            &I_KICKS[from_rot][to_rot]
        } else {
            &JLSTZ_KICKS[from_rot][to_rot]
        };

        // P1 Optimization: Test all 5 kick positions in one GPU batch
        let success_mask = self.batch_kick_test(bus.piece_x, bus.piece_y, kicks)?;

        for (idx, &(dx, dy)) in kicks.iter().enumerate() {
            let bit_set = (success_mask & (1 << idx)) != 0;
            if bit_set {
                let test_x = bus.piece_x + dx;
                let test_y = bus.piece_y + dy;
                bus.piece_x = test_x;
                bus.piece_y = test_y;
                bus.piece_rotation = to_rot as u8;
                bus.wires.collision_any = false;
                bus.wires.wall_kick_applied = dx != 0 || dy != 0;
                bus.wires.render_dirty = true;
                return Ok(());
            }
        }

        Ok(())
    }

    pub(super) fn run_movement_chip(&mut self, bus: &mut SystemBus) -> Result<(), String> {
        let pt = bus.piece_type.0;
        let pr = bus.piece_rotation;

        if bus.wires.hard_drop_requested {
            let mut gy = bus.piece_y;
            while gy < BOARD_ROWS as i8 + 4 {
                if self.piece_collides_cfg(bus, bus.piece_x, gy + 1, pt, pr)? {
                    break;
                }
                gy += 1;
            }
            bus.piece_y = gy;
            bus.wires.piece_locked = true;
            bus.wires.render_dirty = true;
            return Ok(());
        }

        let dx = bus.wires.dx;
        if dx != 0 {
            let new_x = bus.piece_x + dx;
            if !self.piece_collides_cfg(bus, new_x, bus.piece_y, pt, pr)? {
                bus.piece_x = new_x;
                bus.wires.collision_any = false;
                bus.wires.render_dirty = true;
            }
        }

        let should_drop = bus.wires.dy != 0 || bus.wires.gravity_tick;
        if should_drop {
            let new_y = bus.piece_y + 1;
            if !self.piece_collides_cfg(bus, bus.piece_x, new_y, pt, pr)? {
                bus.piece_y = new_y;
                bus.wires.collision_any = false;
                bus.wires.render_dirty = true;
            }
        }

        bus.wires.collision_down =
            self.piece_collides_cfg(bus, bus.piece_x, bus.piece_y + 1, pt, pr)?;
        Ok(())
    }

    pub(super) fn run_lock_delay_timer_chip(
        &mut self,
        pins: &InputPins,
        bus: &mut SystemBus,
    ) -> Result<(), String> {
        let cannot_fall = bus.wires.collision_down && bus.wires.dy == 0;
        let moved =
            bus.wires.dx != 0 || bus.wires.dy != 0 || bus.wires.rotate_cw || bus.wires.rotate_ccw;

        if moved && !cannot_fall {
            bus.lock_delay_accumulator_ns = 0;
            bus.wires.lock_delay_active = false;
            bus.wires.lock_delay_expired = false;
        } else if cannot_fall && !bus.wires.piece_locked {
            bus.wires.lock_delay_active = true;
            bus.lock_delay_accumulator_ns = bus
                .lock_delay_accumulator_ns
                .saturating_add(pins.frame_delta_ns);
            if bus.lock_delay_accumulator_ns >= bus.lock_delay_max_ns {
                bus.wires.lock_delay_expired = true;
            }
        } else {
            bus.lock_delay_accumulator_ns = 0;
            bus.wires.lock_delay_active = false;
            bus.wires.lock_delay_expired = false;
        }

        Ok(())
    }

    pub(super) fn run_piece_locker_chip(&mut self, bus: &mut SystemBus) -> Result<(), String> {
        let should_lock = bus.wires.piece_locked || bus.wires.lock_delay_expired;
        if !should_lock {
            return Ok(());
        }

        let pt = bus.piece_type.0 as usize;
        let pr = bus.piece_rotation as usize;
        let cells = &TETROMINOES[pt][pr];
        let color = bus.piece_type.0 + 1;

        for &(dx, dy) in cells {
            let bx = bus.piece_x + dx;
            let by = bus.piece_y + dy;
            if by < 0 || by >= BOARD_ROWS as i8 || bx < 0 || bx >= BOARD_COLS as i8 {
                bus.wires.game_over_triggered = true;
                return Ok(());
            }
            bus.board[by as usize][bx as usize] = Cell(color);
        }

        bus.wires.piece_locked = true;
        bus.wires.should_spawn_next = true;
        bus.wires.render_dirty = true;
        self.board_synced = false; // P2: Mark board as out-of-sync
        Ok(())
    }

    pub(super) fn run_ghost_chip(&mut self, bus: &mut SystemBus) -> Result<(), String> {
        self.upload_board(bus)?;
        self.upload_piece_cells(bus.piece_type.0, bus.piece_rotation)?;

        // P0 Optimization: Use batch ghost_y scan instead of loop
        let gy = self.ghost_y_scan(bus.piece_x, bus.piece_y)?;

        bus.ghost_x = bus.piece_x;
        bus.ghost_y = gy;
        Ok(())
    }

    pub(super) fn run_line_clear_detector_chip(
        &mut self,
        bus: &mut SystemBus,
    ) -> Result<(), String> {
        self.upload_board(bus)?;

        let mut mask: u32 = 0;
        let mut count: u8 = 0;

        for row in 0..BOARD_ROWS {
            if self.row_is_full(row as i32)? {
                mask |= 1 << row;
                count = count.saturating_add(1);
            }
        }

        bus.wires.full_row_mask = mask;
        bus.wires.lines_cleared_this_tick = count;
        Ok(())
    }

    pub(super) fn run_line_clear_committer_chip(
        &mut self,
        bus: &mut SystemBus,
    ) -> Result<(), String> {
        let mask = bus.wires.full_row_mask;
        if mask == 0 {
            return Ok(());
        }

        let mut dst = (BOARD_ROWS - 1) as i8;
        for src in (0..BOARD_ROWS as i8).rev() {
            if mask & (1 << src) == 0 {
                if dst != src {
                    bus.board[dst as usize] = bus.board[src as usize];
                }
                dst -= 1;
            }
        }

        for y in 0..=dst {
            bus.board[y as usize] = [Cell(0); BOARD_COLS];
        }

        bus.wires.render_dirty = true;
        self.board_synced = false; // P2: Mark board as out-of-sync after clearing lines
        Ok(())
    }

    pub(super) fn run_score_keeper_chip(&mut self, bus: &mut SystemBus) -> Result<(), String> {
        let lines = bus.wires.lines_cleared_this_tick;
        if lines == 0 {
            return Ok(());
        }

        const BASES: [u32; 5] = [0, 40, 100, 300, 1200];
        let idx = (lines as usize).min(4);
        let added = BASES[idx] * (bus.level as u32 + 1);
        bus.score += added;
        bus.lines_cleared += lines as u32;
        Ok(())
    }

    pub(super) fn run_level_calculator_chip(&mut self, bus: &mut SystemBus) -> Result<(), String> {
        let new_level = 1 + (bus.lines_cleared / 10) as u16;
        if new_level != bus.level {
            bus.level = new_level;
            bus.gravity_interval_ns = gravity_interval_ns(new_level as u8);
        }
        Ok(())
    }

    pub(super) fn run_hold_controller_chip(&mut self, bus: &mut SystemBus) -> Result<(), String> {
        if !bus.wires.hold_requested || bus.hold_used {
            return Ok(());
        }

        let old_type = bus.piece_type;
        if let Some(held) = bus.hold_piece_type {
            bus.piece_type = held;
        } else {
            bus.piece_type = bus.next_piece_type;
            bus.next_piece_type = PieceType(Self::next_rand(&mut bus.prng_state));
        }
        bus.hold_piece_type = Some(old_type);
        bus.hold_used = true;
        bus.piece_x = 3;
        bus.piece_y = 0;
        bus.piece_rotation = 0;
        bus.wires.render_dirty = true;
        Ok(())
    }

    pub(super) fn run_spawn_controller_chip(&mut self, bus: &mut SystemBus) -> Result<(), String> {
        if !bus.wires.should_spawn_next {
            return Ok(());
        }

        bus.piece_type = bus.next_piece_type;
        bus.piece_x = 3;
        bus.piece_y = 0;
        bus.piece_rotation = 0;
        bus.hold_used = false;

        if self.piece_collides_cfg(bus, 3, 0, bus.piece_type.0, 0)? {
            bus.wires.game_over_triggered = true;
            return Ok(());
        }

        bus.next_piece_type = PieceType(Self::next_rand(&mut bus.prng_state));
        bus.wires.should_spawn_next = false;
        bus.wires.render_dirty = true;
        Ok(())
    }

    fn upload_board(&mut self, bus: &SystemBus) -> Result<(), String> {
        // P2: Only upload if not synced
        if self.board_synced {
            return Ok(());
        }

        let mut flat = [0u8; BOARD_COLS * BOARD_ROWS];
        for y in 0..BOARD_ROWS {
            for x in 0..BOARD_COLS {
                flat[y * BOARD_COLS + x] = bus.board[y][x].0;
            }
        }
        self.board.copy_from(&flat).map_err(|e| e.to_string())?;
        self.board_synced = true;
        Ok(())
    }

    fn upload_piece_cells(&mut self, piece_type: u8, rotation: u8) -> Result<(), String> {
        let cells = &TETROMINOES[piece_type as usize][rotation as usize];
        let mut packed = [0i32; 8];
        for (i, (dx, dy)) in cells.iter().copied().enumerate() {
            packed[i * 2] = dx as i32;
            packed[i * 2 + 1] = dy as i32;
        }
        self.piece_cells
            .copy_from(&packed)
            .map_err(|e| e.to_string())
    }

    fn piece_collides(&mut self, test_x: i8, test_y: i8) -> Result<bool, String> {
        let module = &self.module;
        let stream = &self.stream;
        unsafe {
            launch!(module.piece_collides_u8<<<1, 1, 0, stream>>>(
                self.board.as_device_ptr(),
                self.piece_cells.as_device_ptr(),
                test_x as i32,
                test_y as i32,
                self.scalar_out.as_device_ptr()
            ))
        }
        .map_err(|e| e.to_string())?;

        self.stream.synchronize().map_err(|e| e.to_string())?;
        let mut out = [0u32; 1];
        self.scalar_out
            .copy_to(&mut out)
            .map_err(|e| e.to_string())?;
        Ok(out[0] != 0)
    }

    fn row_is_full(&mut self, row: i32) -> Result<bool, String> {
        let module = &self.module;
        let stream = &self.stream;
        unsafe {
            launch!(module.row_full_u8<<<1, 1, 0, stream>>>(
                self.board.as_device_ptr(),
                row,
                self.scalar_out.as_device_ptr()
            ))
        }
        .map_err(|e| e.to_string())?;

        self.stream.synchronize().map_err(|e| e.to_string())?;
        let mut out = [0u32; 1];
        self.scalar_out
            .copy_to(&mut out)
            .map_err(|e| e.to_string())?;
        Ok(out[0] != 0)
    }

    fn piece_collides_cfg(
        &mut self,
        bus: &SystemBus,
        test_x: i8,
        test_y: i8,
        piece_type: u8,
        rotation: u8,
    ) -> Result<bool, String> {
        self.upload_board(bus)?;
        self.upload_piece_cells(piece_type, rotation)?;
        self.piece_collides(test_x, test_y)
    }

    fn next_rand(state: &mut u32) -> u8 {
        *state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        ((*state >> 16) % 7) as u8
    }

    /// P0 Optimization: Scan from piece_y downward to find ghost_y in one GPU pass
    fn ghost_y_scan(&mut self, piece_x: i8, piece_y: i8) -> Result<i8, String> {
        let module = &self.module;
        let stream = &self.stream;
        unsafe {
            launch!(module.ghost_y_scan<<<1, 1, 0, stream>>>(
                self.board.as_device_ptr(),
                self.piece_cells.as_device_ptr(),
                piece_x as i32,
                piece_y as i32,
                self.scalar_out.as_device_ptr()
            ))
        }
        .map_err(|e| e.to_string())?;

        self.stream.synchronize().map_err(|e| e.to_string())?;
        let mut out = [0u32; 1];
        self.scalar_out
            .copy_to(&mut out)
            .map_err(|e| e.to_string())?;
        Ok(out[0] as i8)
    }

    /// P1 Optimization: Test 5 wall kick positions in one GPU pass, return success mask
    fn batch_kick_test(
        &mut self,
        piece_x: i8,
        piece_y: i8,
        kicks: &[(i8, i8); 5],
    ) -> Result<u32, String> {
        let mut kicks_packed = [0i32; 10];
        for (i, (dx, dy)) in kicks.iter().enumerate() {
            kicks_packed[i * 2] = *dx as i32;
            kicks_packed[i * 2 + 1] = *dy as i32;
        }

        let mut kicks_buf = DeviceBuffer::from_slice(&kicks_packed).map_err(|e| e.to_string())?;

        let module = &self.module;
        let stream = &self.stream;
        unsafe {
            launch!(module.batch_kick_test<<<1, 1, 0, stream>>>(
                self.board.as_device_ptr(),
                self.piece_cells.as_device_ptr(),
                piece_x as i32,
                piece_y as i32,
                kicks_buf.as_device_ptr(),
                self.scalar_out.as_device_ptr()
            ))
        }
        .map_err(|e| e.to_string())?;

        self.stream.synchronize().map_err(|e| e.to_string())?;
        let mut out = [0u32; 1];
        self.scalar_out
            .copy_to(&mut out)
            .map_err(|e| e.to_string())?;
        Ok(out[0])
    }
}
