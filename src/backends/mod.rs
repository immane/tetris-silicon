// ============================================================================
// backends/mod.rs — BackendRuntime and backend selection
// ============================================================================

use crate::bus::{InputPins, SystemBus};
use crate::chips::Chip;

mod cpu;

#[cfg(feature = "cuda")]
mod cuda;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendKind {
    Cpu,
    #[cfg(feature = "cuda")]
    Cuda,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChipBackendRoute {
    pub layer: usize,
    pub chip: &'static str,
    pub backend: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChipId {
    InputDecoder,
    GravityTimer,
    DasTimer,
    LockDelayTimer,
    CollisionDetector,
    Rotation,
    Movement,
    PieceLocker,
    LineClearDetector,
    LineClearCommitter,
    ScoreKeeper,
    LevelCalculator,
    HoldController,
    SpawnController,
    GhostComputer,
}

impl ChipId {
    pub const COUNT: usize = 15;

    pub const fn index(self) -> usize {
        self as usize
    }
}

pub struct BackendRuntime {
    #[cfg_attr(not(feature = "cuda"), allow(dead_code))]
    kind: BackendKind,
    /// Human-readable backend label, e.g. "cpu" or "cuda [RTX 4090]"
    label: String,
    /// Counts successful GPU-dispatched chip ticks.
    gpu_tick_count: u64,
    /// Per-tick chip routing plan (which chip executed on which backend).
    chip_routes: Vec<ChipBackendRoute>,
    #[cfg(feature = "cuda")]
    cuda: Option<cuda::CudaRuntime>,
}

impl BackendRuntime {
    pub fn cpu() -> Self {
        Self {
            kind: BackendKind::Cpu,
            label: "cpu".to_string(),
            gpu_tick_count: 0,
            chip_routes: Vec::new(),
            #[cfg(feature = "cuda")]
            cuda: None,
        }
    }

    pub fn from_env() -> Self {
        match std::env::var("TETRIS_BACKEND").ok().as_deref() {
            Some("cuda") => {
                #[cfg(feature = "cuda")]
                {
                    return Self::cuda_or_cpu();
                }

                #[cfg(not(feature = "cuda"))]
                {
                    eprintln!(
                        "[backend] requested cuda, but feature is disabled; falling back to cpu"
                    );
                    return Self::cpu();
                }
            }
            _ => Self::cpu(),
        }
    }

    pub fn backend_name(&self) -> &str {
        &self.label
    }

    pub fn gpu_tick_count(&self) -> u64 {
        self.gpu_tick_count
    }

    pub fn chip_backend_routes(&self) -> &[ChipBackendRoute] {
        &self.chip_routes
    }

    pub fn execute_layers(&mut self, layers: &[Vec<Chip>], pins: &InputPins, bus: &mut SystemBus) {
        #[cfg(feature = "cuda")]
        {
            if self.kind == BackendKind::Cuda {
                match self.execute_layers_cuda_mixed(layers, pins, bus) {
                    Ok((dispatched, routes)) => {
                        self.gpu_tick_count = self.gpu_tick_count.wrapping_add(dispatched);
                        self.chip_routes = routes;
                        return;
                    }
                    Err(err) => {
                        eprintln!("[backend] cuda tick failed ({err}); switching to cpu");
                        self.kind = BackendKind::Cpu;
                        self.label = "cpu (cuda fallback)".to_string();
                        self.cuda = None;
                    }
                }
            }
        }

        self.chip_routes.clear();
        self.chip_routes
            .reserve(layers.iter().map(|l| l.len()).sum::<usize>());
        cpu::execute_layers_cpu(layers, pins, bus);
        for (layer_idx, layer) in layers.iter().enumerate() {
            for chip in layer {
                self.chip_routes.push(ChipBackendRoute {
                    layer: layer_idx,
                    chip: chip_name(chip),
                    backend: "cpu",
                });
            }
        }
    }
}

pub(super) fn chip_name(chip: &Chip) -> &'static str {
    match chip {
        Chip::InputDecoder(_) => "InputDecoder",
        Chip::GravityTimer(_) => "GravityTimer",
        Chip::DasTimer(_) => "DasTimer",
        Chip::LockDelayTimer(_) => "LockDelayTimer",
        Chip::CollisionDetector(_) => "CollisionDetector",
        Chip::Rotation(_) => "Rotation",
        Chip::Movement(_) => "Movement",
        Chip::PieceLocker(_) => "PieceLocker",
        Chip::LineClearDetector(_) => "LineClearDetector",
        Chip::LineClearCommitter(_) => "LineClearCommitter",
        Chip::ScoreKeeper(_) => "ScoreKeeper",
        Chip::LevelCalculator(_) => "LevelCalculator",
        Chip::HoldController(_) => "HoldController",
        Chip::SpawnController(_) => "SpawnController",
        Chip::GhostComputer(_) => "GhostComputer",
    }
}

#[cfg_attr(not(feature = "cuda"), allow(dead_code))]
pub(super) fn chip_id(chip: &Chip) -> ChipId {
    match chip {
        Chip::InputDecoder(_) => ChipId::InputDecoder,
        Chip::GravityTimer(_) => ChipId::GravityTimer,
        Chip::DasTimer(_) => ChipId::DasTimer,
        Chip::LockDelayTimer(_) => ChipId::LockDelayTimer,
        Chip::CollisionDetector(_) => ChipId::CollisionDetector,
        Chip::Rotation(_) => ChipId::Rotation,
        Chip::Movement(_) => ChipId::Movement,
        Chip::PieceLocker(_) => ChipId::PieceLocker,
        Chip::LineClearDetector(_) => ChipId::LineClearDetector,
        Chip::LineClearCommitter(_) => ChipId::LineClearCommitter,
        Chip::ScoreKeeper(_) => ChipId::ScoreKeeper,
        Chip::LevelCalculator(_) => ChipId::LevelCalculator,
        Chip::HoldController(_) => ChipId::HoldController,
        Chip::SpawnController(_) => ChipId::SpawnController,
        Chip::GhostComputer(_) => ChipId::GhostComputer,
    }
}

impl Default for BackendRuntime {
    fn default() -> Self {
        Self::cpu()
    }
}
