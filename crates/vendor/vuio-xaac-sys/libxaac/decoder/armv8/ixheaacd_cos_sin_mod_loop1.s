.macro push_v_regs
    stp             q8, q9, [sp, #-32]!
    stp             q10, q11, [sp, #-32]!
    stp             q12, q13, [sp, #-32]!
    stp             q14, q15, [sp, #-32]!
    stp             X8, X9, [sp, #-16]!
    stp             X10, X11, [sp, #-16]!
    stp             X12, X13, [sp, #-16]!
    stp             X14, X15, [sp, #-16]!
    stp             X16, X17, [sp, #-16]!
    stp             X18, X19, [sp, #-16]!
    stp             X20, X21, [sp, #-16]!
    stp             X29, X30, [sp, #-16]!
.endm

.macro pop_v_regs
    ldp             X29, X30, [sp], #16
    ldp             X20, X21, [sp], #16
    ldp             X18, X19, [sp], #16
    ldp             X16, X17, [sp], #16
    ldp             X14, X15, [sp], #16
    ldp             X12, X13, [sp], #16
    ldp             X10, X11, [sp], #16
    ldp             X8, X9, [sp], #16
    ldp             q14, q15, [sp], #32
    ldp             q12, q13, [sp], #32
    ldp             q10, q11, [sp], #32
    ldp             q8, q9, [sp], #32
.endm

.text
.p2align 2

     .global ixheaacd_cos_sin_mod_loop1
ixheaacd_cos_sin_mod_loop1:

    // STMFD sp!, {x4-x12, x14}
    push_v_regs
    //stp x19, x20,[sp,#-16]!
    //VPUSH {D8-D11}
//generating load addresses
    ADD             x4, x0, x1, lsl #3  //psubband1
    SUB             x4, x4, #4
    ADD             x5, x3, x1, lsl #3  //psubband1_t
    SUB             x5, x5, #8
    ASR             x6, x1, #2

    MOV             w19, #0
    DUP             V0.8h, w19
LOOP1:
//first part
    ld1             {v0.h}[0] , [x2]
    ADD             x2, x2, #2
    ld1             {v0.h}[2] , [x2]
    ADD             x2, x2, #2
    rev64           v1.2s, v0.2s
    ld1             {v2.s}[0], [x0]
    ADD             x0, x0, #4
    ADD             x7, x0, #252
    ld1             {v2.s}[1], [x7]
    ld1             {v3.s}[0], [x4]
    ADD             x7, x4, #256
    ld1             {v3.s}[1], [x7]
    SUB             x4, x4, #4

    sMULL           v4.2d, v0.2s, v2.2s //qsub 2nd
    sshr            v4.2d, v4.2d, #16
    sMULL           v6.2d, v0.2s, v3.2s //add 2nd
    sshr            v6.2d, v6.2d, #16
    sMULL           v8.2d, v1.2s, v2.2s //add 1st
    sshr            v8.2d, v8.2d, #16
    sMULL           v10.2d, v1.2s, v3.2s //qsub 1st
    sshr            v10.2d, v10.2d, #16

    add             v0.4s, v8.4s , v6.4s
    SQSUB           v2.4s, v10.4s , v4.4s

    //shrn  v0.2s, v0.2d,#32
    //shrn  v2.2s, v2.2d,#32
    mov             v3.16b, v0.16b
    mov             v1.16b, v2.16b
    ST2             {v0.s, v1.s}[0], [x3]
    ADD             x3, x3, #8
    ADD             x7, x3, #248
    ST2             {v2.s, v3.s}[2], [x7]
    MOV             w19, #0
    DUP             V0.8h, w19
//second part
    ld1             {v0.h}[0] , [x2]
    ADD             x2, x2, #2
    ld1             {v0.h}[2] , [x2]
    ADD             x2, x2, #2
    rev64           v1.2s, v0.2s
    ld1             {v2.s}[0], [x0]
    ADD             x0, x0, #4
    ADD             x7, x0, #252
    ld1             {v2.s}[1], [x7]
    ld1             {v3.s}[0], [x4]
    ADD             x7, x4, #256
    ld1             {v3.s}[1], [x7]
    SUB             x4, x4, #4

    sMULL           v4.2d, v0.2s, v2.2s //qsub 2nd
    sshr            v4.2d, v4.2d, #16
    sMULL           v6.2d, v0.2s, v3.2s //add 2nd
    sshr            v6.2d, v6.2d, #16
    sMULL           v8.2d, v1.2s, v2.2s //add 1st
    sshr            v8.2d, v8.2d, #16
    sMULL           v10.2d, v1.2s, v3.2s //qsub 1st
    sshr            v10.2d, v10.2d, #16

    ADD             v0.4s, v10.4s , v4.4s
    SQSUB           v2.4s, v8.4s , v6.4s

    //shrn  v0.2s, v0.2d,#32
    //shrn  v2.2s, v2.2d,#32
    mov             v3.16b, v0.16b
    mov             v1.16b, v2.16b
    ST2             {v0.s, v1.s}[0], [x5]
    ADD             x7, x5, #256
    ST2             {v2.s, v3.s}[2], [x7]
    SUB             x5, x5, #8
    MOV             w19, #0
    DUP             V0.8h, w19
//Third part
    ld1             {v0.h}[0] , [x2]
    ADD             x2, x2, #2
    ld1             {v0.h}[2] , [x2]
    ADD             x2, x2, #2
    rev64           v1.2s, v0.2s
    ld1             {v2.s}[0], [x0], #4
    ADD             x7, x0, #252
    ld1             {v2.s}[1], [x7]
    ld1             {v3.s}[0], [x4]
    ADD             x7, x4, #256
    ld1             {v3.s}[1], [x7]
    SUB             x4, x4, #4

    sMULL           v4.2d, v0.2s, v2.2s //qsub 2nd
    sshr            v4.2d, v4.2d, #16
    sMULL           v6.2d, v0.2s, v3.2s //add 2nd
    sshr            v6.2d, v6.2d, #16
    sMULL           v8.2d, v1.2s, v2.2s //add 1st
    sshr            v8.2d, v8.2d, #16
    sMULL           v10.2d, v1.2s, v3.2s //qsub 1st
    sshr            v10.2d, v10.2d, #16

    add             v0.4s, v8.4s , v6.4s
    SQSUB           v2.4s, v10.4s , v4.4s

    //shrn  v0.2s, v0.2d,#32
    //shrn  v2.2s, v2.2d,#32
    mov             v3.16b, v0.16b
    mov             v1.16b, v2.16b
    ST2             {v0.s, v1.s}[0], [x3]
    ADD             x3, x3, #8
    ADD             x7, x3, #248
    ST2             {v2.s, v3.s}[2], [x7]
    MOV             w19, #0
    DUP             V0.8h, w19
//Fourth part
    ld1             {v0.h}[0] , [x2]
    ADD             x2, x2, #2
    ld1             {v0.h}[2] , [x2]
    ADD             x2, x2, #2
    rev64           v1.2s, v0.2s
    ld1             {v2.s}[0], [x0]
    ADD             x0, x0, #4
    ADD             x7, x0, #252
    ld1             {v2.s}[1], [x7]
    ld1             {v3.s}[0], [x4]
    ADD             x7, x4, #256
    ld1             {v3.s}[1], [x7]
    SUB             x4, x4, #4

    sMULL           v4.2d, v0.2s, v2.2s //qsub 2nd
    sshr            v4.2d, v4.2d, #16
    sMULL           v6.2d, v0.2s, v3.2s //add 2nd
    sshr            v6.2d, v6.2d, #16
    sMULL           v8.2d, v1.2s, v2.2s //add 1st
    sshr            v8.2d, v8.2d, #16
    sMULL           v10.2d, v1.2s, v3.2s //qsub 1st
    sshr            v10.2d, v10.2d, #16


    ADD             v0.4s, v10.4s , v4.4s
    SQSUB           v2.4s, v8.4s , v6.4s

    //shrn  v0.2s, v0.2d,#32
    //shrn  v2.2s, v2.2d,#32
    mov             v3.16b, v0.16b
    mov             v1.16b, v2.16b
    ST2             {v0.s, v1.s}[0], [x5]
    ADD             x7, x5, #256
    SUBS            x6, x6, #1
    ST2             {v2.s, v3.s}[2], [x7]
    SUB             x5, x5, #8
    MOV             w19, #0
    DUP             V0.8h, w19
    BGT             LOOP1
    //VPOP {D8-D11}
    // LDMFD sp!, {x4-x12, x15}
    //ldp x19, x20,[sp],#16
    pop_v_regs
    ret


























