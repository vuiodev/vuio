.macro push_v_regs
    stp             X8, X9, [sp, #-16]!
    stp             X10, X11, [sp, #-16]!
    stp             X12, X13, [sp, #-16]!
    stp             X14, X15, [sp, #-16]!
    stp             X20, X21, [sp, #-16]!
    stp             X26, X17, [sp, #-16]!
    stp             X27, X28, [sp, #-16]!
    stp             q2, q3, [sp, #-32]!
    stp             q0, q1, [sp, #-32]!
.endm
.macro pop_v_regs
    ldp             q0, q1, [sp], #32
    ldp             q2, q3, [sp], #32
    ldp             X27, X28, [sp], #16
    ldp             X26, X17, [sp], #16
    ldp             X20, X21, [sp], #16
    ldp             X14, X15, [sp], #16
    ldp             X12, X13, [sp], #16
    ldp             X10, X11, [sp], #16
    ldp             X8, X9, [sp], #16
.endm

.text
.p2align 2
    .global ixheaacd_shiftrountine_with_rnd
ixheaacd_shiftrountine_with_rnd:
    push_v_regs

    ADD             x12, x2, x3, LSL #1
    MOV             W9, #0x00008000
    DUP             V0.4s, w9
    MOVI            v3.4s, #10
    MOV             W27, #0x80000000
    MOV             W28, #0x7fffffff
    MOV             W26, #0
    SUBS            W3, W3, #1
    BMI             S_WITH_R_L6

S_WITH_R_L5:
    LDR             w5, [x1, x3, LSL #2] //i2 = qmfImag[j]
    LDR             w7, [x0, x3, LSL #2] //x2 = qmfReal[j]
    LDR             w14, [x0], #4       //x1 = *qmfReal
    LDR             w10, [x1], #4       //i1 = *qmfImag

    ADD             w6, w5, w7          //*qmfImag++ = add32(i2, x2)
    SUB             w5, w5, w7          //qmfReal[j] = sub32(i2, x2)
    ADD             w7, w10, w14        //qmfImag[j] = add32(i1, x1)
    SUB             w4, w10, w14        //*qmfReal++ = sub32(i1, x1)

    MOV             v1.s[0], W4         //QADD        x4, x4, x9
    MOV             v1.s[1], W5         //QADD        x4, x4, x9
    MOV             v1.s[2], W6         //QADD        x4, x4, x9
    MOV             v1.s[3], W7         //QADD        x4, x4, x9
    lsl             w14, w3, #1

    SQSHL           v1.4s, v1.4s, v3.4s
    ADD             X17, X2, X14

    SQADD           v2.4s, v1.4s, v0.4s

    ST1             {v2.h}[1], [x2], #2
    ST1             {v2.h}[3], [X17]
    ADD             X17, X12, X14
    ST1             {v2.h}[7], [x17]    //STRH   w7, [x12, x14]
    ST1             {v2.h}[5], [x12], #2 //STRH   w6, [x12], #2

    SUBS            x3, x3, #2

    BGE             S_WITH_R_L5
S_WITH_R_L6:
    pop_v_regs
    ret
