///******************************************************************************
// *
// * Copyright (C) 2018 The Android Open Source Project
// *
// * Licensed under the Apache License, Version 2.0 (the "License");
// * you may not use this file except in compliance with the License.
// * You may obtain a copy of the License at:
// *
// * http://www.apache.org/licenses/LICENSE-2.0
// *
// * Unless required by applicable law or agreed to in writing, software
// * distributed under the License is distributed on an "AS IS" BASIS,
// * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// * See the License for the specific language governing permissions and
// * limitations under the License.
// *
// *****************************************************************************
// * Originally developed and contributed by Ittiam Systems Pvt. Ltd, Bangalore
//*/

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
    stp             X29, X30, [sp, #-16]!
.endm
.macro pop_v_regs
    ldp             X29, X30, [sp], #16
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
.global ixheaacd_over_lap_add1_armv8
ixheaacd_over_lap_add1_armv8:
    push_v_regs
    LSL             X6 , X6 , #1
    LSL             X10, X5, #1
    SUB             X11, X10, #1
    LSL             X10, X11, #2
    ADD             X10, X0, X10
    SUB             X10, X10, #12
    LSL             X8, X11, #1
    ADD             X8, X8, X3
    SUB             X8, X8, #14
    MOV             X12, #-16
    DUP             V11.8H, W4
    LD1             {V3.4S}, [X10], X12

    NEG             W7, W7
    SQNEG           V0.4S, V3.4S
    UZP1            V31.8H, V0.8H, V0.8H
    UZP2            V30.8H, V0.8H, V0.8H
    REV64           V31.8h, V31.8h
    REV64           V30.8h, V30.8h
    SUB             X11, X5, #1
    UZP1            V7.8H, V3.8H, V3.8H
    UZP2            V6.8H, V3.8H, V3.8H
    REV64           V7.8H, V7.8H
    REV64           V6.8H, V6.8H
    MOV             V16.S[0], W6
    MOV             V17.S[0], W11
    SMULL           V17.4S, V16.4H, V17.4H
    MOV             W11, V17.S[0]
    LSL             X11, X11, #1

    LD2             {V2.4H, V3.4H}, [X8], X12
    ADD             X11, X11, X2
    REV64           V2.4H, V2.4H
    REV64           V3.4H, V3.4H
    LSL             X4, X6, #1
    NEG             X4, X4
    LSL             X9, X6, #1
    MOV             V16.S[0], W5
    MOV             V17.S[0], W6
    SMULL           V17.4S, V16.4H, V17.4H
    MOV             W6, V17.S[0]
    LSL             W6, W6, #1
    ADD             X6, X6, X2

    UMULL           V15.4S, V7.4H, V2.4H
    LD1             {V4.4S}, [X1], #16
    USHR            V15.4S, V15.4S, #16

    SMLAL           V15.4S, V6.4H, V2.4H
    SQSHL           V15.4S, V15.4S, V11.4S
    SSHLL           V27.4S, V3.4H, #0
    SMULL           V28.2D, V27.2S, V4.2S
    SMULL2          V29.2D, V27.4S, V4.4S
    SQXTN           V28.2S, V28.2D
    SQXTN2          V28.4S, V29.2D
    MOV             V14.16B, V28.16B

    SQSUB           V13.4S, V15.4S, V14.4S
    UMULL           V12.4S, V31.4H, V3.4H
    USHR            V12.4S, V12.4S, #16
    SMLAL           V12.4S, V30.4H, V3.4H
    SQSHL           V12.4S, V12.4S, V11.4S
    LD1             {V3.4S}, [X10], X12

    SSHLL           V27.4S, V2.4H, #0
    SMULL           V28.2D, V27.2S, V4.2S
    SMULL2          V29.2D, V27.4S, V4.4S
    SQXTN           V28.2S, V28.2D
    SQXTN2          V28.4S, V29.2D
    MOV             V8.16B, V28.16B

    SQNEG           V0.4S, V3.4S
    UZP1            V1.8H, V0.8H, V0.8H
    UZP2            V0.8H, V0.8H, V0.8H
    REV64           V1.8h, V1.8h
    REV64           V0.8h, V0.8h
    SQSUB           V9.4S, V12.4S, V8.4S
    UZP1            V7.8H, V3.8H, V3.8H
    UZP2            V6.8H, V3.8H, V3.8H
    REV64           V7.8h, V7.8h
    REV64           V6.8h, V6.8h
    LD2             {V2.4H, V3.4H}, [X8], X12
    REV64           V2.4H, V2.4H
    REV64           V3.4H, V3.4H

    LD1             {V4.4S}, [X1], #16
    SUB             W5, W5, #8


LOOP_1:

    ST1             {V13.S}[0], [X11], X4
    UMULL           V15.4S, V7.4H, V2.4H
    ST1             {V13.S}[1], [X11], X4
    UMULL           V12.4S, V1.4H, V3.4H
    ST1             {V13.S}[2], [X11], X4
    USHR            V15.4S, V15.4S, #16
    ST1             {V13.S}[3], [X11], X4
    USHR            V12.4S, V12.4S, #16
    ST1             {V9.S}[0], [X6], X9
    SMLAL           V15.4S, V6.4H, V2.4H
    ST1             {V9.S}[1], [X6], X9
    SMLAL           V12.4S, V0.4H, V3.4H
    ST1             {V9.S}[2], [X6], X9
    SQSHL           V15.4S, V15.4S, V11.4S
    ST1             {V9.S}[3], [X6], X9
    SQSHL           V12.4S, V12.4S, V11.4S
    LD1             {V6.4S}, [X10], X12

    SSHLL           V27.4S, V3.4H, #0
    SMULL           V28.2D, V27.2S, V4.2S
    SMULL2          V29.2D, V27.4S, V4.4S
    SQXTN           V28.2S, V28.2D
    SQXTN2          V28.4S, V29.2D
    MOV             V14.16B, V28.16B

    SSHLL           V27.4S, V2.4H, #0
    SMULL           V28.2D, V27.2S, V4.2S
    SMULL2          V29.2D, V27.4S, V4.4S
    SQXTN           V28.2S, V28.2D
    SQXTN2          V28.4S, V29.2D
    MOV             V8.16B, V28.16B

    LD2             {V2.4H, V3.4H}, [X8], X12

    SQNEG           V0.4S, V6.4S

    LD1             {V4.4S}, [X1], #16

    UZP1            V1.8H, V0.8H, V0.8H
    UZP2            V0.8H, V0.8H, V0.8H
    REV64           V1.8h, V1.8h
    REV64           V0.8h, V0.8h
    UZP1            V7.8H, V6.8H, V6.8H
    UZP2            V6.8H, V6.8H, V6.8H
    REV64           V7.8h, V7.8h
    REV64           V6.8h, V6.8h
    SQSUB           V13.4S, V15.4S, V14.4S
    REV64           V2.4H, V2.4H
    REV64           V3.4H, V3.4H
    SQSUB           V9.4S, V12.4S, V8.4S
    UMULL           V15.4S, V7.4H, V2.4H
    ST1             {V13.S}[0], [X11], X4
    UMULL           V12.4S, V1.4H, V3.4H
    USHR            V15.4S, V15.4S, #16
    ST1             {V13.S}[1], [X11], X4
    SMLAL           V15.4S, V6.4H, V2.4H
    ST1             {V13.S}[2], [X11], X4
    USHR            V12.4S, V12.4S, #16
    ST1             {V13.S}[3], [X11], X4
    SMLAL           V12.4S, V0.4H, V3.4H
    ST1             {V9.S}[0], [X6], X9
    SQSHL           V15.4S, V15.4S, V11.4S
    ST1             {V9.S}[1], [X6], X9
    SQSHL           V12.4S, V12.4S, V11.4S
    ST1             {V9.S}[2], [X6], X9

    SSHLL           V27.4S, V3.4H, #0
    SMULL           V28.2D, V27.2S, V4.2S
    SMULL2          V29.2D, V27.4S, V4.4S
    SQXTN           V28.2S, V28.2D
    SQXTN2          V28.4S, V29.2D
    MOV             V14.16B, V28.16B

    ST1             {V9.S}[3], [X6], X9


    SSHLL           V27.4S, V2.4H, #0
    SMULL           V28.2D, V27.2S, V4.2S
    SMULL2          V29.2D, V27.4S, V4.4S
    SQXTN           V28.2S, V28.2D
    SQXTN2          V28.4S, V29.2D
    MOV             V8.16B, V28.16B

    LD1             {V3.4S}, [X10], X12

    SQNEG           V0.4S, V3.4S
    UZP1            V1.8H, V0.8H, V0.8H
    UZP2            V0.8H, V0.8H, V0.8H
    REV64           V1.8H, V1.8H
    REV64           V0.8H, V0.8H
    SQSUB           V13.4S, V15.4S, V14.4S
    UZP1            V7.8H, V3.8H, V3.8H
    UZP2            V6.8H, V3.8H, V3.8H
    REV64           V7.8H, V7.8H
    REV64           V6.8H, V6.8H

    LD2             {V2.4H, V3.4H}, [X8], X12
    SQSUB           V9.4S, V12.4S, V8.4S
    REV64           V2.4H, V2.4H
    REV64           V3.4H, V3.4H

    LD1             {V4.4S}, [X1], #16


    SUBS            X5, X5, #8

    BGT             LOOP_1

    ST1             {V13.S}[0], [X11], X4
    UMULL           V15.4S, V7.4H, V2.4H
    ST1             {V13.S}[1], [X11], X4
    UMULL           V12.4s, V1.4H, V3.4H
    ST1             {V13.S}[2], [X11], X4
    USHR            V15.4S, V15.4S, #16
    ST1             {V13.S}[3], [X11], X4
    USHR            V12.4S, V12.4S, #16

    ST1             {V9.S}[0], [X6], X9
    SMLAL           V15.4S, V6.4H, V2.4H
    ST1             {V9.S}[1], [X6], X9
    SMLAL           V12.4S, V0.4H, V3.4H
    ST1             {V9.S}[2], [X6], X9
    SQSHL           V15.4S, V15.4S, V11.4S
    ST1             {V9.S}[3], [X6], X9
    SQSHL           V12.4S, V12.4S, V11.4S


    SSHLL           V27.4S, V3.4H, #0
    SMULL           V28.2D, V27.2S, V4.2S
    SMULL2          V29.2D, V27.4S, V4.4S
    SQXTN           V28.2S, V28.2D
    SQXTN2          V28.4S, V29.2D
    MOV             V14.16B, V28.16B

    SSHLL           V27.4S, V2.4H, #0
    SMULL           V28.2D, V27.2S, V4.2S
    SMULL2          V29.2D, V27.4S, V4.4S
    SQXTN           V28.2S, V28.2D
    SQXTN2          V28.4S, V29.2D
    MOV             V8.16B, V28.16B


    SQSUB           V13.4S, V15.4S, V14.4S
    SQSUB           V9.4S, V12.4S, V8.4S


    ST1             {V13.S}[0], [X11], X4
    ST1             {V13.S}[1], [X11], X4
    ST1             {V13.S}[2], [X11], X4
    ST1             {V13.S}[3], [X11], X4

    ST1             {V9.S}[0], [X6], X9
    ST1             {V9.S}[1], [X6], X9
    ST1             {V9.S}[2], [X6], X9
    ST1             {V9.S}[3], [X6], X9
    pop_v_regs
    RET




