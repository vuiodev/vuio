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

     .global ixheaacd_cos_sin_mod_loop2
ixheaacd_cos_sin_mod_loop2:

    // STMFD sp!, {x4-x12, x14}
    push_v_regs
    //stp x19, x20,[sp,#-16]!
    //VPUSH {D8-D15}
    //generating load addresses
    ADD             x3, x0, x2, LSL #3  //psubband1 = &subband[2 * M - 1];
    SUB             x3, x3, #4
    ADD             x10, x0, #256
    ADD             x11, x10, x2, LSL #3
    SUB             x11, x11, #4
    MOV             x8, #-4
    MOV             w19, #0
    DUP             V0.4s, w19
    DUP             V1.4s, w19

    LDR             w6, [x0]
    sxtw            x6, w6
    ASR             x4, x2, #1          //M_2 = ixheaacd_shx32(M, 1);
    SUB             x4, x4, #1

    ASR             x6, x6, #1          //*psubband = *psubband >> 1;
    LD1             {v2.s}[0], [x3]

    STR             w6, [x0], #4        //psubband++;
    sxtw            x6, w6
    LDR             w7, [x0]
    sxtw            x7, w7
    ASR             x7, x7, #1
    sub             x20, x7, #0
    neg             x6, x20
    STR             w6, [x3], #-4
    sxtw            x6, w6
    LD1             {v3.s}[0], [x3]     //  im = *psubband1;

    LD2             {v0.h, v1.h}[0], [x1], #4
    sxtl            v0.4s, v0.4h
    sxtl            v1.4s, v1.4h
    dup             v0.2s, v0.s[0]
    dup             v1.2s, v1.s[0]

    LD1             {v2.s}[1], [x11]    //re = *psubband12;

//    LDR w6,  [x10]
//  sxtw x6,w6
//    ASR x7, x6, #1
//    MOV x9, #0
//    QSUB x7, x9, x7
    LD1             {v4.s}[0], [x10]
    SSHR            v4.2s, v4.2s, #1
    MOV             x9, #0
    DUP             v6.2s, w9
    SQSUB           v4.2s, v6.2s, v4.2s

    ST1             {v4.s}[0], [x11]
//  str     X7, [X11]
    SUB             x11, x11, #4
//  sxtw x7,w7

    LDR             w6, [x10, #4]
    sxtw            x6, w6
    ASR             x6, x6, #1
    STR             w6, [x10], #4
    sxtw            x6, w6

    LD1             {v3.s}[1], [x11]

    sMULL           v4.2d, v0.2s, v2.2s //qsub 2nd
    sshr            v4.2d, v4.2d, #16
    sMULL           v6.2d, v0.2s, v3.2s //add 2nd
    sshr            v6.2d, v6.2d, #16
    sMULL           v8.2d, v1.2s, v2.2s //add 1st
    sshr            v8.2d, v8.2d, #16
    sMULL           v10.2d, v1.2s, v3.2s //qsub 1st
    sshr            v10.2d, v10.2d, #16

    add             v12.2d, v8.2d , v6.2d
    SQSUB           v14.2d, v10.2d , v4.2d
    SQSUB           v16.2d, v4.2d , v10.2d

    //shrn  v12.2s, v12.2d,#32
    //shrn  v14.2s, v14.2d,#32
    //shrn  v16.2s, v16.2d,#32

    ST1             {v12.s}[0], [x3], x8

    ST1             {v14.s}[0], [x0], #4

    SQNEG           v12.4s, v12.4s


    ST1             {v12.s}[2], [x10], #4

    ST1             {v16.s}[2], [x11], x8

LOOP1:
    LD1             {v2.2s}, [x0]
    LD1             {v3.2s}, [x10]
    LDR             w5, [x3]            //RE2
    sxtw            x5, w5
    LDR             w6, [x11]           //RE3
    sxtw            x6, w6
    //VTRN.32 D2, D3
    TRN1            v4.2s, v2.2s, v3.2s
    TRN2            v3.2s, v2.2s, v3.2s
    MOV             v2.8b, v4.8b

    sMULL           v4.2d, v0.2s, v2.2s //qsub 2nd
    sshr            v4.2d, v4.2d, #16
    sMULL           v6.2d, v0.2s, v3.2s //add 2nd
    sshr            v6.2d, v6.2d, #16
    sMULL           v8.2d, v1.2s, v2.2s //add 1st
    sshr            v8.2d, v8.2d, #16
    sMULL           v10.2d, v1.2s, v3.2s //qsub 1st
    sshr            v10.2d, v10.2d, #16

    add             v12.2d, v8.2d , v6.2d
    SQSUB           v14.2d, v4.2d , v10.2d
    SQSUB           v16.2d, v10.2d , v4.2d

    //shrn  v12.2s, v12.2d,#32
    //shrn  v14.2s, v14.2d,#32
    //shrn  v16.2s, v16.2d,#32

    ST1             {v12.s}[0], [x0], #4
    ST1             {v14.s}[0], [x3], x8
    SQNEG           v12.4s, v12.4s

    ST1             {v12.s}[2], [x11], x8
    ST1             {v16.s}[2], [x10], #4

    MOV             w19, #0
    DUP             V0.4s, w19
    DUP             V1.4s, w19
    // second part
    LD2             {v0.h, v1.h}[0], [x1], #4
    sxtl            v0.4s, v0.4h
    sxtl            v1.4s, v1.4h
    dup             v0.2s, v0.s[0]
    dup             v1.2s, v1.s[0]

    mov             v3.s[0], w5
    mov             v3.s[1], w6
    LD1             {v2.s}[0], [x3]
    LD1             {v2.s}[1], [x11]

    sMULL           v4.2d, v0.2s, v2.2s //qsub 2nd
    sshr            v4.2d, v4.2d, #16
    sMULL           v6.2d, v0.2s, v3.2s //add 2nd
    sshr            v6.2d, v6.2d, #16
    sMULL           v8.2d, v1.2s, v2.2s //add 1st
    sshr            v8.2d, v8.2d, #16
    sMULL           v10.2d, v1.2s, v3.2s //qsub 1st
    sshr            v10.2d, v10.2d, #16

    add             v12.2d, v4.2d , v10.2d
    SQSUB           v14.2d, v8.2d , v6.2d
    SQSUB           v16.2d, v6.2d , v8.2d

    //shrn  v12.2s, v12.2d,#32
    //shrn  v14.2s, v14.2d,#32
    //shrn  v16.2s, v16.2d,#32

    ST1             {v12.s}[0], [x3], x8
    ST1             {v14.s}[0], [x0], #4

    SQNEG           v12.4s, v12.4s

    subs            x4, x4, #1
    ST1             {v12.s}[2], [x10], #4
    ST1             {v16.s}[2], [x11], x8

    BGT             LOOP1
    //VPOP {D8-D15}
    // LDMFD sp!, {x4-x12, x15}
    //ldp x19, x20,[sp],#16
    pop_v_regs
    ret
