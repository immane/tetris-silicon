use rustacuda::memory::DeviceBuffer;
use rustacuda::prelude::*;

mod kernels;
mod ptx;
mod runtime;

pub(super) struct CudaRuntime {
    pub(super) device_name: String,
    pub(super) module: Module,
    pub(super) stream: Stream,
    pub(super) board: DeviceBuffer<u8>,
    pub(super) piece_cells: DeviceBuffer<i32>,
    pub(super) scalar_out: DeviceBuffer<u32>,
    // Keep CUDA context as the last field so it drops after stream/module/buffers.
    pub(super) _context: Context,
}
