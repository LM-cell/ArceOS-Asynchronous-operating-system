use std::arch::global_asm;

global_asm!(
    r#"
    .intel_syntax noprefix
    .global gt_switch
    .type gt_switch, @function
gt_switch:
    mov [rdi + 0x00], rsp
    mov [rdi + 0x08], r15
    mov [rdi + 0x10], r14
    mov [rdi + 0x18], r13
    mov [rdi + 0x20], r12
    mov [rdi + 0x28], rbx
    mov [rdi + 0x30], rbp

    mov rsp, [rsi + 0x00]
    mov r15, [rsi + 0x08]
    mov r14, [rsi + 0x10]
    mov r13, [rsi + 0x18]
    mov r12, [rsi + 0x20]
    mov rbx, [rsi + 0x28]
    mov rbp, [rsi + 0x30]
    ret
    .att_syntax prefix
    "#
);

extern "C" {
    pub fn gt_switch(old: *mut Context, new: *const Context);
}

#[repr(C)]
#[derive(Default)]
pub struct Context {
    rsp: usize,
    r15: usize,
    r14: usize,
    r13: usize,
    r12: usize,
    rbx: usize,
    rbp: usize,
}

impl Context {
    pub fn with_rsp(rsp: usize) -> Self {
        Self {
            rsp,
            ..Self::default()
        }
    }
}
