use super::ptx::CUDA_MIXED_PTX;
use super::CudaRuntime;
use crate::backends::{chip_id, chip_name, BackendKind, BackendRuntime, ChipBackendRoute, ChipId};
use crate::bus::{InputPins, SystemBus, BOARD_COLS, BOARD_ROWS};
use crate::chips::{Chip, LogicChip};

use rustacuda::memory::DeviceBuffer;
use rustacuda::prelude::*;

use std::ffi::CString;

/// GPU kernel implementations available. All other chips run on CPU via trait interface.
const CUDA_KERNEL_CHIPS: [ChipId; 4] = [
    ChipId::CollisionDetector,
    ChipId::LineClearDetector,
    ChipId::GhostComputer,
    ChipId::Rotation, // P1: Batch wall kick test optimization
];

/// Contract default: chips eligible for CUDA routing (GPU kernel or CPU emulation).
const CONTRACT_CUDA_CHIPS: [ChipId; 15] = [
    ChipId::InputDecoder,
    ChipId::GravityTimer,
    ChipId::DasTimer,
    ChipId::LockDelayTimer,
    ChipId::CollisionDetector,
    ChipId::Rotation,
    ChipId::Movement,
    ChipId::PieceLocker,
    ChipId::LineClearDetector,
    ChipId::LineClearCommitter,
    ChipId::ScoreKeeper,
    ChipId::LevelCalculator,
    ChipId::HoldController,
    ChipId::SpawnController,
    ChipId::GhostComputer,
];

#[derive(Clone, Copy, Debug)]
struct CudaRoutingPlan {
    /// true = attempt CUDA first; false = run on CPU directly
    cuda_enabled: [bool; ChipId::COUNT],
}

impl CudaRoutingPlan {
    fn contract_default() -> Self {
        let mut plan = Self {
            cuda_enabled: [false; ChipId::COUNT],
        };
        // Default: enable all chips in contract (GPU kernels + CPU emulation).
        // Individual chips will decide execution mode in run_chip_on_cuda.
        for chip in &CONTRACT_CUDA_CHIPS {
            plan.cuda_enabled[chip.index()] = true;
        }
        plan
    }

    fn from_env() -> Self {
        let mut plan = Self::contract_default();
        let Some(raw) = std::env::var("TETRIS_CUDA_CHIPS").ok() else {
            return plan;
        };

        let spec = raw.trim().to_ascii_lowercase();
        if spec.is_empty() {
            return plan;
        }

        if spec == "none" {
            plan.cuda_enabled = [false; ChipId::COUNT];
            return plan;
        }

        if spec == "all" {
            plan.cuda_enabled = [true; ChipId::COUNT];
            return plan;
        }

        if spec == "contract" {
            return plan;
        }

        // Comma-separated list of chip names:
        // e.g. TETRIS_CUDA_CHIPS=CollisionDetector,GhostComputer
        plan.cuda_enabled = [false; ChipId::COUNT];
        for token in spec.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            if let Some(chip) = parse_chip_name(token) {
                plan.cuda_enabled[chip.index()] = true;
            }
        }

        plan
    }

    fn can_run_on_cuda(&self, chip: ChipId) -> bool {
        self.cuda_enabled[chip.index()]
    }
}

impl BackendRuntime {
    pub(crate) fn cuda_or_cpu() -> Self {
        match CudaRuntime::try_new() {
            Ok(runtime) => {
                let label = format!("cuda [{}]", runtime.device_name);
                eprintln!("[backend] running on {label}");
                Self {
                    kind: BackendKind::Cuda,
                    label,
                    gpu_tick_count: 0,
                    chip_routes: Vec::new(),
                    cuda: Some(runtime),
                }
            }
            Err(err) => {
                eprintln!("[backend] cuda runtime init failed ({err}); falling back to cpu");
                Self::cpu()
            }
        }
    }

    pub(crate) fn execute_layers_cuda_mixed(
        &mut self,
        layers: &[Vec<Chip>],
        pins: &InputPins,
        bus: &mut SystemBus,
    ) -> Result<(u64, Vec<ChipBackendRoute>), String> {
        let cuda = self
            .cuda
            .as_mut()
            .ok_or_else(|| "missing cuda runtime".to_string())?;

        let mut dispatched: u64 = 0;
        let mut routes = Vec::with_capacity(layers.iter().map(|l| l.len()).sum::<usize>());
        let plan = CudaRoutingPlan::from_env();

        for (layer_idx, layer) in layers.iter().enumerate() {
            for chip in layer {
                let chip_kind = chip_id(chip);
                let backend_used = if plan.can_run_on_cuda(chip_kind) {
                    if has_gpu_kernel(chip_kind) {
                        match run_chip_on_cuda(cuda, chip, pins, bus) {
                            Ok(()) => {
                                dispatched = dispatched.wrapping_add(1);
                                "gpu-kernel"
                            }
                            Err(err) => {
                                eprintln!(
                                    "[backend] chip {} failed on cuda ({}); falling back to trait-cpu for this chip",
                                    chip_name(chip), err
                                );
                                chip.tick(pins, bus);
                                if chip_mutates_board(chip_kind) {
                                    cuda.board_synced = false;
                                }
                                "trait-cpu"
                            }
                        }
                    } else {
                        chip.tick(pins, bus);
                        if chip_mutates_board(chip_kind) {
                            cuda.board_synced = false;
                        }
                        "trait-cpu"
                    }
                } else {
                    chip.tick(pins, bus);
                    if chip_mutates_board(chip_kind) {
                        cuda.board_synced = false;
                    }
                    "cpu"
                };

                routes.push(ChipBackendRoute {
                    layer: layer_idx,
                    chip: chip_name(chip),
                    backend: backend_used,
                });
            }
        }

        Ok((dispatched, routes))
    }
}

fn parse_chip_name(name: &str) -> Option<ChipId> {
    match name {
        "inputdecoder" | "input_decoder" => Some(ChipId::InputDecoder),
        "gravitytimer" | "gravity_timer" => Some(ChipId::GravityTimer),
        "dastimer" | "das_timer" => Some(ChipId::DasTimer),
        "lockdelaytimer" | "lock_delay_timer" => Some(ChipId::LockDelayTimer),
        "collisiondetector" | "collision_detector" => Some(ChipId::CollisionDetector),
        "rotation" => Some(ChipId::Rotation),
        "movement" => Some(ChipId::Movement),
        "piecelocker" | "piece_locker" => Some(ChipId::PieceLocker),
        "linecleardetector" | "line_clear_detector" => Some(ChipId::LineClearDetector),
        "lineclearcommitter" | "line_clear_committer" => Some(ChipId::LineClearCommitter),
        "scorekeeper" | "score_keeper" => Some(ChipId::ScoreKeeper),
        "levelcalculator" | "level_calculator" => Some(ChipId::LevelCalculator),
        "holdcontroller" | "hold_controller" => Some(ChipId::HoldController),
        "spawncontroller" | "spawn_controller" => Some(ChipId::SpawnController),
        "ghostcomputer" | "ghost_computer" => Some(ChipId::GhostComputer),
        _ => None,
    }
}

fn has_gpu_kernel(chip_kind: ChipId) -> bool {
    CUDA_KERNEL_CHIPS.contains(&chip_kind)
}

fn chip_mutates_board(chip_kind: ChipId) -> bool {
    matches!(chip_kind, ChipId::PieceLocker | ChipId::LineClearCommitter)
}

fn run_chip_on_cuda(
    cuda: &mut CudaRuntime,
    chip: &Chip,
    pins: &InputPins,
    bus: &mut SystemBus,
) -> Result<(), String> {
    let chip_kind = chip_id(chip);
    if has_gpu_kernel(chip_kind) {
        // Execute GPU kernel for supported chips
        match chip_kind {
            ChipId::CollisionDetector => cuda.run_collision_chip(bus),
            ChipId::LineClearDetector => cuda.run_line_clear_detector_chip(bus),
            ChipId::GhostComputer => cuda.run_ghost_chip(bus),
            ChipId::Rotation => cuda.run_rotation_chip(bus), // P1: GPU batch wall kick test
            _ => unreachable!("has_gpu_kernel must match"),
        }
    } else {
        // No GPU kernel: run on CPU via trait interface
        chip.tick(pins, bus);
        Ok(())
    }
}

impl CudaRuntime {
    fn try_new() -> Result<Self, String> {
        rustacuda::init(CudaFlags::empty()).map_err(|e| e.to_string())?;

        let device = Device::get_device(0).map_err(|e| e.to_string())?;
        let device_name = device.name().map_err(|e| e.to_string())?;
        let context =
            Context::create_and_push(ContextFlags::MAP_HOST | ContextFlags::SCHED_AUTO, device)
                .map_err(|e| e.to_string())?;

        let ptx = CString::new(CUDA_MIXED_PTX).map_err(|e| e.to_string())?;
        let module = Module::load_from_string(&ptx).map_err(|e| e.to_string())?;
        let stream = Stream::new(StreamFlags::NON_BLOCKING, None).map_err(|e| e.to_string())?;

        let board =
            DeviceBuffer::from_slice(&[0u8; BOARD_COLS * BOARD_ROWS]).map_err(|e| e.to_string())?;
        let piece_cells = DeviceBuffer::from_slice(&[0i32; 8]).map_err(|e| e.to_string())?;
        let scalar_out = DeviceBuffer::from_slice(&[0u32]).map_err(|e| e.to_string())?;

        Ok(Self {
            device_name,
            module,
            stream,
            board,
            piece_cells,
            scalar_out,
            board_synced: false, // P2: Initially not synced
            _context: context,
        })
    }
}
