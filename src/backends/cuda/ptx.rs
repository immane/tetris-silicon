pub(super) const CUDA_MIXED_PTX: &str = r#"
.version 5.0
.target sm_30
.address_size 64

.visible .entry piece_collides_u8(
    .param .u64 board_ptr,
    .param .u64 piece_cells_ptr,
    .param .s32 test_x,
    .param .s32 test_y,
    .param .u64 out_ptr
)
{
    .reg .pred %p<8>;
    .reg .b32 %r<24>;
    .reg .b64 %rd<16>;

    ld.param.u64 %rd1, [board_ptr];
    ld.param.u64 %rd2, [piece_cells_ptr];
    ld.param.s32 %r1, [test_x];
    ld.param.s32 %r2, [test_y];
    ld.param.u64 %rd3, [out_ptr];

    // cell 0
    ld.global.s32 %r3, [%rd2+0];
    ld.global.s32 %r4, [%rd2+4];
    add.s32 %r5, %r1, %r3;
    add.s32 %r6, %r2, %r4;
    setp.lt.s32 %p1, %r5, 0;
    @%p1 bra COLLIDE;
    setp.ge.s32 %p2, %r5, 10;
    @%p2 bra COLLIDE;
    setp.lt.s32 %p3, %r6, 0;
    @%p3 bra COLLIDE;
    setp.ge.s32 %p4, %r6, 20;
    @%p4 bra COLLIDE;
    mul.lo.s32 %r7, %r6, 10;
    add.s32 %r8, %r7, %r5;
    cvt.u64.u32 %rd4, %r8;
    add.u64 %rd5, %rd1, %rd4;
    ld.global.u8 %r9, [%rd5];
    setp.ne.s32 %p5, %r9, 0;
    @%p5 bra COLLIDE;

    // cell 1
    ld.global.s32 %r3, [%rd2+8];
    ld.global.s32 %r4, [%rd2+12];
    add.s32 %r5, %r1, %r3;
    add.s32 %r6, %r2, %r4;
    setp.lt.s32 %p1, %r5, 0;
    @%p1 bra COLLIDE;
    setp.ge.s32 %p2, %r5, 10;
    @%p2 bra COLLIDE;
    setp.lt.s32 %p3, %r6, 0;
    @%p3 bra COLLIDE;
    setp.ge.s32 %p4, %r6, 20;
    @%p4 bra COLLIDE;
    mul.lo.s32 %r7, %r6, 10;
    add.s32 %r8, %r7, %r5;
    cvt.u64.u32 %rd4, %r8;
    add.u64 %rd5, %rd1, %rd4;
    ld.global.u8 %r9, [%rd5];
    setp.ne.s32 %p5, %r9, 0;
    @%p5 bra COLLIDE;

    // cell 2
    ld.global.s32 %r3, [%rd2+16];
    ld.global.s32 %r4, [%rd2+20];
    add.s32 %r5, %r1, %r3;
    add.s32 %r6, %r2, %r4;
    setp.lt.s32 %p1, %r5, 0;
    @%p1 bra COLLIDE;
    setp.ge.s32 %p2, %r5, 10;
    @%p2 bra COLLIDE;
    setp.lt.s32 %p3, %r6, 0;
    @%p3 bra COLLIDE;
    setp.ge.s32 %p4, %r6, 20;
    @%p4 bra COLLIDE;
    mul.lo.s32 %r7, %r6, 10;
    add.s32 %r8, %r7, %r5;
    cvt.u64.u32 %rd4, %r8;
    add.u64 %rd5, %rd1, %rd4;
    ld.global.u8 %r9, [%rd5];
    setp.ne.s32 %p5, %r9, 0;
    @%p5 bra COLLIDE;

    // cell 3
    ld.global.s32 %r3, [%rd2+24];
    ld.global.s32 %r4, [%rd2+28];
    add.s32 %r5, %r1, %r3;
    add.s32 %r6, %r2, %r4;
    setp.lt.s32 %p1, %r5, 0;
    @%p1 bra COLLIDE;
    setp.ge.s32 %p2, %r5, 10;
    @%p2 bra COLLIDE;
    setp.lt.s32 %p3, %r6, 0;
    @%p3 bra COLLIDE;
    setp.ge.s32 %p4, %r6, 20;
    @%p4 bra COLLIDE;
    mul.lo.s32 %r7, %r6, 10;
    add.s32 %r8, %r7, %r5;
    cvt.u64.u32 %rd4, %r8;
    add.u64 %rd5, %rd1, %rd4;
    ld.global.u8 %r9, [%rd5];
    setp.ne.s32 %p5, %r9, 0;
    @%p5 bra COLLIDE;

    mov.u32 %r20, 0;
    st.global.u32 [%rd3], %r20;
    ret;

COLLIDE:
    mov.u32 %r20, 1;
    st.global.u32 [%rd3], %r20;
    ret;
}

.visible .entry row_full_u8(
    .param .u64 board_ptr,
    .param .s32 row,
    .param .u64 out_ptr
)
{
    .reg .pred %p<2>;
    .reg .b32 %r<16>;
    .reg .b64 %rd<8>;

    ld.param.u64 %rd1, [board_ptr];
    ld.param.s32 %r1, [row];
    ld.param.u64 %rd2, [out_ptr];

    mul.lo.s32 %r2, %r1, 10;

    add.s32 %r3, %r2, 0;
    cvt.u64.u32 %rd3, %r3;
    add.u64 %rd4, %rd1, %rd3;
    ld.global.u8 %r4, [%rd4];
    setp.eq.s32 %p1, %r4, 0;
    @%p1 bra ROW_EMPTY;

    add.s32 %r3, %r2, 1;
    cvt.u64.u32 %rd3, %r3;
    add.u64 %rd4, %rd1, %rd3;
    ld.global.u8 %r4, [%rd4];
    setp.eq.s32 %p1, %r4, 0;
    @%p1 bra ROW_EMPTY;

    add.s32 %r3, %r2, 2;
    cvt.u64.u32 %rd3, %r3;
    add.u64 %rd4, %rd1, %rd3;
    ld.global.u8 %r4, [%rd4];
    setp.eq.s32 %p1, %r4, 0;
    @%p1 bra ROW_EMPTY;

    add.s32 %r3, %r2, 3;
    cvt.u64.u32 %rd3, %r3;
    add.u64 %rd4, %rd1, %rd3;
    ld.global.u8 %r4, [%rd4];
    setp.eq.s32 %p1, %r4, 0;
    @%p1 bra ROW_EMPTY;

    add.s32 %r3, %r2, 4;
    cvt.u64.u32 %rd3, %r3;
    add.u64 %rd4, %rd1, %rd3;
    ld.global.u8 %r4, [%rd4];
    setp.eq.s32 %p1, %r4, 0;
    @%p1 bra ROW_EMPTY;

    add.s32 %r3, %r2, 5;
    cvt.u64.u32 %rd3, %r3;
    add.u64 %rd4, %rd1, %rd3;
    ld.global.u8 %r4, [%rd4];
    setp.eq.s32 %p1, %r4, 0;
    @%p1 bra ROW_EMPTY;

    add.s32 %r3, %r2, 6;
    cvt.u64.u32 %rd3, %r3;
    add.u64 %rd4, %rd1, %rd3;
    ld.global.u8 %r4, [%rd4];
    setp.eq.s32 %p1, %r4, 0;
    @%p1 bra ROW_EMPTY;

    add.s32 %r3, %r2, 7;
    cvt.u64.u32 %rd3, %r3;
    add.u64 %rd4, %rd1, %rd3;
    ld.global.u8 %r4, [%rd4];
    setp.eq.s32 %p1, %r4, 0;
    @%p1 bra ROW_EMPTY;

    add.s32 %r3, %r2, 8;
    cvt.u64.u32 %rd3, %r3;
    add.u64 %rd4, %rd1, %rd3;
    ld.global.u8 %r4, [%rd4];
    setp.eq.s32 %p1, %r4, 0;
    @%p1 bra ROW_EMPTY;

    add.s32 %r3, %r2, 9;
    cvt.u64.u32 %rd3, %r3;
    add.u64 %rd4, %rd1, %rd3;
    ld.global.u8 %r4, [%rd4];
    setp.eq.s32 %p1, %r4, 0;
    @%p1 bra ROW_EMPTY;

    mov.u32 %r10, 1;
    st.global.u32 [%rd2], %r10;
    ret;

ROW_EMPTY:
    mov.u32 %r10, 0;
    st.global.u32 [%rd2], %r10;
    ret;
}
"#;
