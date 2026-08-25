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
.global ixheaacd_over_lap_add2_armv8


ixheaacd_over_lap_add2_armv8:
    push_v_regs
    MOV             X8, X5
    SUB             X12, X5, #1
    LSL             X9, X5, #2
    LSL             X12, X12, #2
    ADD             X10, X0, X9
    ADD             X7, X1, X12
    ADD             X4, X4, #1
    LD2             {V0.4H, V1.4H}, [X10], #16
    LSL             X11, X6, #2
    SUB             X7, X7, #12
    SUB             X4, X4, #16
    MOV             X12, #-16
    MOV             X13, #1
    ADD             X14, X4, #1
    NEG             X14, X14
    DUP             V21.4S, W4
    LD2             {V6.4H, V7.4H}, [X7], X12
    LSL             X4, X13, X14
    REV64           V4.4H, V6.4H
    DUP             V20.4S, W4
    REV64           V5.4H, V7.4H
    MOV             X4, X3

    MOV             X9, X2
    LD2             {V2.4H, V3.4H}, [X3], #16

    UMULL           V23.4S, V0.4H, V2.4H
    UMLSL           V23.4S, V4.4H, V3.4H
    LD2             {V8.4H, V9.4H}, [X10], #16
    SSHR            V23.4S, V23.4S, #16
    LD2             {V10.4H, V11.4H}, [X3], #16
    SMLAL           V23.4S, V1.4H, V2.4H
    SMLSL           V23.4S, V5.4H, V3.4H
    LD2             {V14.4H, V15.4H}, [X7], X12
    REV64           V12.4H, V14.4H
    REV64           V13.4H, V15.4H
    SQADD           V22.4S, V23.4S, V20.4S
    SSHL            V22.4S, V22.4S, V21.4S
    MOV             V24.16B, V22.16B
    SUB             X8, X8, #8

LOOP_1:

    LD2             {V0.4H, V1.4H}, [X10], #16
    UMULL           V19.4S, V8.4H, V10.4H
    LD2             {V2.4H, V3.4H}, [X3], #16
    UMLSL           V19.4S, V12.4H, V11.4H
    LD2             {V6.4H, V7.4H}, [X7], X12
    UMULL           V23.4S, V0.4H, V2.4H
    REV64           V4.4H, V6.4H
    UMLSL           V23.4S, V4.4H, V3.4H
    REV64           V5.4H, V7.4H
    SSHR            V19.4S, V19.4S, #16
    ST1             {V24.S}[0], [X2], X11
    SMLAL           V19.4S, V9.4H, V10.4H
    ST1             {V24.S}[1], [X2], X11
    SSHR            V23.4S, V23.4S, #16
    ST1             {V24.S}[2], [X2], X11
    SMLAL           V23.4S, V1.4H, V2.4H

    ST1             {V24.S}[3], [X2], X11
    SMLSL           V19.4S, V13.4H, V11.4H
    SMLSL           V23.4S, V5.4H, V3.4H

    LD2             {V8.4H, V9.4H}, [X10], #16
    LD2             {V10.4H, V11.4H}, [X3], #16


    LD2             {V14.4H, V15.4H}, [X7], X12
    SQADD           V18.4S, V19.4S, V20.4S
    REV64           V12.4H, V14.4H
    REV64           V13.4H, V15.4H
    SQADD           V22.4S, V23.4S, V20.4S
    SSHL            V18.4S, V18.4S, V21.4S
    MOV             V16.16B, V18.16B
    ST1             {V16.S}[0], [X2], X11
    SSHL            V22.4S, V22.4S, V21.4S


    MOV             V24.16B, V22.16B
    SUBS            X8, X8, #8

    ST1             {V16.S}[1], [X2], X11
    ST1             {V16.S}[2], [X2], X11
    ST1             {V16.S}[3], [X2], X11


    BGT             LOOP_1


    ST1             {V24.S}[0], [X2], X11
    UMULL           V19.4S, V8.4H, V10.4H
    UMLSL           V19.4S, V12.4H, V11.4H
    ST1             {V24.S}[1], [X2], X11
    ST1             {V24.S}[2], [X2], X11
    SSHR            V19.4S, V19.4S, #16
    ST1             {V24.S}[3], [X2], X11
    SMLAL           V19.4S, V9.4H, V10.4H
    SMLSL           V19.4S, V13.4H, V11.4H
    MOV             X12, #12
    MOV             V30.S[0], W5
    MOV             V31.S[0], W6
    SMULL           V29.4S, V30.4H, V31.4H
    MOV             W7, V29.S[0]

    LSL             W10, W5, #1
    SQADD           V18.4S, V19.4S, V20.4S
    SSHL            V18.4S, V18.4S, V21.4S
    MOV             V16.16B, V18.16B

    ST1             {V16.S}[0], [X2], X11
    LSL             X7, X7, #2

    ST1             {V16.S}[1], [X2], X11
    ADD             X7, X7, X9

    ST1             {V16.S}[2], [X2], X11
    ST1             {V16.S}[3], [X2], X11

    SUB             X11, X10, #1
    LSL             X10, X11, #2
    ADD             X10, X0, X10
    LSL             X11, X11, #1
    SUB             X10, X10, X12
    LSL             X8, X6, #2
    MOV             X12, #-16
    ADD             X11, X11, X4

    LD1             {V6.4S}, [X10], X12
    SUB             X11, X11, #14


    REV64           V0.4S, V6.4S
    SQNEG           V0.4S, V0.4S


    UZP1            V1.8H, V0.8H, V0.8H
    UZP2            V0.8H, V0.8H, V0.8H
    REV64           V1.4S, V1.4S
    REV64           V0.4S, V0.4S
    LD2             {V2.4H, V3.4H}, [X11], X12
    REV64           V2.4H, V2.4H
    REV64           V3.4H, V3.4H

    LD2             {V4.4H, V5.4H}, [X1], #16

    UMULL           V23.4S, V1.4H, V3.4H
    UMLSL           V23.4S, V4.4H, V2.4H
    SSHR            V23.4S, V23.4S, #16
    SMLAL           V23.4S, V0.4H, V3.4H
    SMLSL           V23.4S, V5.4H, V2.4H
    SQADD           V22.4S, V23.4S, V20.4S
    SSHL            V22.4S, V22.4S, V21.4S
    MOV             V24.16B, V22.16B


    LD1             {V14.4S}, [X10], X12
    UMULL           V23.4S, V1.4H, V3.4H
    UMLSL           V23.4S, V4.4H, V2.4H
    REV64           V8.4S, V14.4S
    SQNEG           V8.4S, V8.4S
    LD2             {V10.4H, V11.4H}, [X11], X12
    SSHR            V23.4S, V23.4S, #16
    LD2             {V12.4H, V13.4H}, [X1], #16
    SMLAL           V23.4S, V0.4H, V3.4H
    SMLSL           V23.4S, V5.4H, V2.4H
    UZP1            V9.8H, V8.8H, V8.8H
    UZP2            V8.8H, V8.8H, V8.8H
    rev64           v9.4s, v9.4s
    rev64           v8.4s, v8.4s
    REV64           V10.4H, V10.4H
    REV64           V11.4H, V11.4H
    SQADD           V22.4S, V23.4S, V20.4S
    SUB             X5, X5, #8
    SSHL            V22.4S, V22.4S, V21.4S
    MOV             V24.16B, V22.16B


LOOP_2:


    LD1             {V6.4S}, [X10], X12
    UMULL           V19.4S, V9.4H, V11.4H
    REV64           V0.4S, V6.4S
    SQNEG           V0.4S, V0.4S
    UZP1            V1.8H, V0.8H, V0.8H
    UZP2            V0.8H, V0.8H, V0.8H
    REV64           V1.4S, V1.4S
    REV64           V0.4S, V0.4S
    LD2             {V2.4H, V3.4H}, [X11], X12
    REV64           V2.8H, V2.8H
    REV64           V3.8H, V3.8H

    LD2             {V4.4H, V5.4H}, [X1], #16
    UMLSL           V19.4S, V12.4H, V10.4H
    ST1             {V24.S}[0], [X7], X8
    UMULL           V23.4S, V1.4H, V3.4H
    ST1             {V24.S}[1], [X7], X8
    SSHR            V19.4S, V19.4S, #16
    ST1             {V24.S}[2], [X7], X8
    UMLSL           V23.4S, V4.4H, V2.4H
    ST1             {V24.S}[3], [X7], X8
    SMLAL           V19.4S, V8.4H, V11.4H
    LD1             {V14.4S}, [X10], X12
    SSHR            V23.4S, V23.4S, #16
    SMLSL           V19.4S, V13.4H, V10.4H
    LD2             {V10.4H, V11.4H}, [X11], X12
    SMLAL           V23.4S, V0.4H, V3.4H
    SMLSL           V23.4S, V5.4H, V2.4H
    REV64           V8.4S, V14.4S
    LD2             {V12.4H, V13.4H}, [X1], #16
    SQNEG           V8.4S, V8.4S
    REV64           V11.4H, V11.4h
    REV64           V10.4H, V10.4H
    SQADD           V18.4S, V19.4S, V20.4S
    UZP1            V9.8H, V8.8H, V8.8H
    UZP2            V8.8H, V8.8H, V8.8H
    rev64           v9.4s, v9.4s
    rev64           v8.4s, v8.4s
    SQADD           V22.4S, V23.4S, V20.4S
    SSHL            V18.4S, V18.4S, V21.4S
    SUBS            X5, X5, #8
    MOV             V16.16B, V18.16B
    ST1             {V16.S}[0], [X7], X8
    SSHL            V22.4S, V22.4S, V21.4S
    ST1             {V16.S}[1], [X7], X8
    MOV             V24.16B, V22.16B

    ST1             {V16.S}[2], [X7], X8
    ST1             {V16.S}[3], [X7], X8

    BGT             LOOP_2

    ST1             {V24.S}[0], [X7], X8
    UMULL           V19.4S, V9.4H, V11.4H
    UMLSL           V19.4S, V12.4H, V10.4H
    ST1             {V24.S}[1], [X7], X8
    ST1             {V24.S}[2], [X7], X8
    SSHR            V19.4S, V19.4S, #16
    ST1             {V24.S}[3], [X7], X8

    SMLAL           V19.4S, V8.4H, V11.4H
    SMLSL           V19.4S, V13.4H, V10.4H
    SQADD           V18.4S, V19.4S, V20.4S
    SSHL            V18.4S, V18.4S, V21.4S
    MOV             V16.16B, V18.16B

    ST1             {V16.S}[0], [X7], X8
    ST1             {V16.S}[1], [X7], X8
    ST1             {V16.S}[2], [X7], X8
    ST1             {V16.S}[3], [X7], X8

    pop_v_regs
    RET
