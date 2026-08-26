.macro push_v_regs
    stp             d8, d9, [sp, #-16]!
    stp             d10, d11, [sp, #-16]!
    stp             d12, d13, [sp, #-16]!
    stp             d14, d15, [sp, #-16]!
    stp             X8, X9, [sp, #-16]!
    stp             X10, X11, [sp, #-16]!
    stp             X12, X13, [sp, #-16]!
    stp             X14, X15, [sp, #-16]!
    stp             X16, X17, [sp, #-16]!
    stp             X29, X30, [sp, #-16]!
.endm
.macro pop_v_regs
    ldp             X29, X30, [sp], #16
    ldp             X16, X17, [sp], #16
    ldp             X14, X15, [sp], #16
    ldp             X12, X13, [sp], #16
    ldp             X10, X11, [sp], #16
    ldp             X8, X9, [sp], #16
    ldp             d14, d15, [sp], #16
    ldp             d12, d13, [sp], #16
    ldp             d10, d11, [sp], #16
    ldp             d8, d9, [sp], #16
.endm

.text
.p2align 2
    .global ixheaacd_sbr_qmfanal32_winadds

ixheaacd_sbr_qmfanal32_winadds:         // PROC

    // STMFD sp!, {x4-x12, x14}
    push_v_regs
    stp             x19, x20, [sp, #-16]!
    //VPUSH       {D8 - D15}
    //LDR w5,  [SP, #108]              //filterStates
    //sxtw x5,w5
    //LDR w6,  [SP, #112]              //timeIn
    //sxtw x6,w6
    //LDR w7,  [SP, #116]              //stride
    //sxtw x7,w7

    LSL             x9, x7, #1


    MOV             x20, x4
    ADD             x5, x5, #64
    MOV             w10, #3

    //ADD         x5, x5, #56
    //MOV         x10, #1
    ////SUB         x6, x6, x9
    //CMP           x7, #1
    //MOV         x11, #-8
    //BGT         LOOP_SKIP_ODD

LOOP:
    LDRSH           w4  , [x6]
    ADD             x6, x6, x9
    LDRSH           w8  , [x6]
    ADD             x6, x6, x9
    LDRSH           w11  , [x6]
    ADD             x6, x6, x9
    LDRSH           w12 , [x6]
    ADD             x6, x6, x9

    STRH            w4  , [x5 , #-2]!
    STRH            w8  , [x5 , #-2]!
    STRH            w11  , [x5 , #-2]!
    STRH            w12 , [x5 , #-2]!

    LDRSH           w4  , [x6]
    ADD             x6, x6, x9
    LDRSH           w8  , [x6]
    ADD             x6, x6, x9
    LDRSH           w11  , [x6]
    ADD             x6, x6, x9
    LDRSH           w12 , [x6]
    ADD             x6, x6, x9

    STRH            w4  , [x5 , #-2]!
    STRH            w8  , [x5 , #-2]!
    STRH            w11  , [x5 , #-2]!
    STRH            w12 , [x5 , #-2]!
    SUBS            w10, w10, #1

    BPL             LOOP


//LOOP:
//  LD1 {v0.4h} , [x6], #8
//  LD1 {v1.4h} , [x6], #8
//
//  REV64  v4.4h , v0.4h
//  REV64  v5.4h , v1.4h
//
//  ST1 {v4.4h} , [x5] , x11
//  ST1 {v5.4h} , [x5] , x11
//
//  LD1 {v2.4h} , [x6], #8
//  LD1 {v3.4h} , [x6], #8
//
//  REV64  v6.4h , v2.4h
//  REV64  v7.4h , v3.4h
//
//  ST1 {v6.4h} , [x5] , x11
//  ST1 {v7.4h} , [x5] , x11
//
//    SUBS        x10, x10, #1
//    BPL         LOOP
//  B       SKIP_LOOP
//
//LOOP_SKIP_ODD:
//  LD2 {v0.4h , v1.4h} , [x6], #16
//  LD2 {v2.4h , v3.4h} , [x6], #16
//
//  REV64  v1.4h , v0.4h
//  REV64  v3.4h , v2.4h
//
//  ST1 {v1.4h} , [x5], x11
//  ST1 {v3.4h} , [x5], x11
//
//  LD2 {v4.4h , v5.4h} , [x6], #16
//  LD2 {v6.4h , v7.4h} , [x6], #16
//
//
//  REV64  v5.4h , v4.4h
//  REV64  v7.4h , v6.4h
//
//  ST1 {v5.4h} , [x5], x11
//  ST1 {v7.4h} , [x5], x11
//
//    SUBS        x10, x10, #1
//    BPL         LOOP_SKIP_ODD

SKIP_LOOP:

    //LDR w4,  [SP, #104]              //winAdd
    // sxtw x4,w4

    MOV             x4, x20
    MOV             x5, #8
    LD1             {v0.4h}, [x0], #8
    MOV             x6, #64

    LSL             x6, x6, #1
    LD2             {v1.4h, v2.4h}, [x2], #16
    MOV             x7, #244

    MOV             x9, x0
    ADD             x0, x0, #120

    MOV             x11, x4
    LD1             {v2.4h}, [x0], x6
    ADD             x11, x11, #128




    MOV             x10, x2
    ADD             x2, x2, #240

    sMULL           v30.4s, v0.4h, v1.4h
    LD2             {v3.4h, v4.4h}, [x2], #16
    ADD             x2, x2, #240


    LD1             {v4.4h}, [x0], x6
    sMLAL           v30.4s, v2.4h, v3.4h

    LD2             {v5.4h, v6.4h}, [x2], #16


    ADD             x2, x2, #240
    LD1             {v6.4h}, [x0], x6
    sMLAL           v30.4s, v4.4h, v5.4h

    LD2             {v7.4h, v8.4h}, [x2], #16


    ADD             x2, x2, #240
    LD1             {v8.4h}, [x0], x6
    sMLAL           v30.4s, v6.4h, v7.4h

    MOV             x0, x9
    LD2             {v9.4h, v10.4h}, [x2], #16


    ADD             x2, x2, #240
    LD1             {v10.4h}, [x1], #8
    sMLAL           v30.4s, v8.4h, v9.4h



    MOV             x9, x1
    LD2             {v11.4h, v12.4h}, [x3], #16
    ADD             x1, x1, #120


    MOV             x2, x10
    LD1             {v12.4h}, [x1], x6
    MOV             x10, x3

    ADD             x3, x3, #240
    LD2             {v13.4h, v14.4h}, [x3], #16
    ADD             x3, x3, #240


    LD2             {v15.4h, v16.4h}, [x3], #16

    LD1             {v14.4h}, [x1], x6
    ADD             x3, x3, #240



    LD1             {v16.4h}, [x1], x6
    SUB             x5, x5, #1

    LD2             {v17.4h, v18.4h}, [x3], #16


    ADD             x3, x3, #240
    LD1             {v18.4h}, [x1], x6

    MOV             x1, x9
    LD2             {v19.4h, v20.4h}, [x3], #16

    ADD             x3, x3, #240

    MOV             x3, x10


LOOP_1:


    LD1             {v0.4h}, [x0], #8

    MOV             x9, x0
    LD2             {v1.4h, v2.4h}, [x2], #16
    ADD             x0, x0, #120

    MOV             x10, x2
    ST1             { v30.4s}, [x4], #16
    ADD             x2, x2, #240


    sMULL           v30.4s, v10.4h, v11.4h
    LD1             {v2.4h}, [x0], x6
    sMLAL           v30.4s, v12.4h, v13.4h

    sMLAL           v30.4s, v14.4h, v15.4h
    LD2             {v3.4h, v4.4h}, [x2], #16
    sMLAL           v30.4s, v16.4h, v17.4h

    sMLAL           v30.4s, v18.4h, v19.4h
    LD1             {v4.4h}, [x0], x6
    ADD             x2, x2, #240

    ST1             { v30.4s}, [x11], #16


    sMULL           v30.4s, v0.4h, v1.4h
    LD2             {v5.4h, v6.4h}, [x2], #16
    sMLAL           v30.4s, v2.4h, v3.4h



    ADD             x2, x2, #240
    LD1             {v6.4h}, [x0], x6
    sMLAL           v30.4s, v4.4h, v5.4h

    LD2             {v7.4h, v8.4h}, [x2], #16


    ADD             x2, x2, #240
    LD1             {v8.4h}, [x0], x6
    sMLAL           v30.4s, v6.4h, v7.4h

    MOV             x0, x9
    LD2             {v9.4h, v10.4h}, [x2], #16



    ADD             x2, x2, #240
    LD1             {v10.4h}, [x1], #8
    MOV             x2, x10

    MOV             x9, x1
    LD2             {v11.4h, v12.4h}, [x3], #16
    ADD             x1, x1, #120


    sMLAL           v30.4s, v8.4h, v9.4h
    LD1             {v12.4h}, [x1], x6
    MOV             x10, x3


    ADD             x3, x3, #240
    LD2             {v13.4h, v14.4h}, [x3], #16
    ADD             x3, x3, #240



    LD1             {v14.4h}, [x1], x6
    LD2             {v15.4h, v16.4h}, [x3], #16
    ADD             x3, x3, #240


    LD1             {v16.4h}, [x1], x6
    LD2             {v17.4h, v18.4h}, [x3], #16
    ADD             x3, x3, #240


    LD1             {v18.4h}, [x1], x6
    SUBS            x5, x5, #1

    MOV             x1, x9
    LD2             {v19.4h, v20.4h}, [x3], #16

    ADD             x3, x3, #240

    MOV             x3, x10

    BGT             LOOP_1

    ST1             { v30.4s}, [x4], #16
    sMULL           v30.4s, v10.4h, v11.4h
    sMLAL           v30.4s, v12.4h, v13.4h

    sMLAL           v30.4s, v14.4h, v15.4h
    sMLAL           v30.4s, v16.4h, v17.4h
    sMLAL           v30.4s, v18.4h, v19.4h

    ST1             { v30.4s}, [x11], #16

    //VPOP        {D8 - D15}
    // LDMFD sp!, {x4-x12, x15}
    ldp             x19, x20, [sp], #16
    pop_v_regs
    ret
    // ENDP
