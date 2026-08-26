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
    stp             X22, X23, [sp, #-16]!
    stp             X16, X17, [sp, #-16]!
    stp             X20, X21, [sp, #-16]!
.endm
.macro pop_v_regs
    ldp             X20, X21, [sp], #16
    ldp             X16, X17, [sp], #16
    ldp             X22, X23, [sp], #16
    ldp             X12, X13, [sp], #16
    ldp             X10, X11, [sp], #16
    ldp             X8, X9, [sp], #16
    ldp             q14, q15, [sp], #32
    ldp             q12, q13, [sp], #32
    ldp             q10, q11, [sp], #32
    ldp             q8, q9, [sp], #32
.endm

.macro swp reg1, reg2
    MOV             X16, \reg1
    MOV             \reg1, \reg2
    MOV             \reg2, x16
.endm
.text
.global ixheaacd_pretwiddle_compute_armv8

ixheaacd_pretwiddle_compute_armv8:

    push_v_regs

    LSL             x7, x4, #4
    ADD             x7, x2, x7
    SUB             x7, x7, #4
    MOV             x22, #7500
    ADD             x3, x3, x22
    MVN             w5, w5
    ADD             w5, w5, #1





ARM_PROLOGUE:
    LDRH            w21, [x3]
    LDRH            w22, [x3, #2]
    LSL             w22, w22, #16
    LSL             w21, w21, #16

    LDR             w8, [x3], #4
    LDR             w9, [x0], #4












    SMULL           X12, w9, w21
    ASR             X12, x12, #32
    LDR             w10, [x1], #-4
    SMULL           X11, w9, w22
    ASR             X11, x11, #32
    SMULL           X23, w10, w22
    ASR             X23, x23, #32
    ADD             w9, w12, w23
    SMULL           X6, w10, w21
    ASR             X6, x6, #32


    MVN             w9, w9
    ADD             w9, w9, #1
    SUB             w11, w11, w6
    CMP             w5, #0
    BGT             NEXT
    MVN             w8, w5
    ADD             w8, w8, #1
    ASR             w11, w11, w8
    ASR             w9, w9, w8
    B               NEXT1

NEXT:
    LSL             w11, w11, w5
    LSL             w9, w9, w5



NEXT1:
    STR             w9, [x2], #4
    STR             w11, [x2], #4

    CMP             X4, #0x100
    BNE             NXT
    MOV             X6, #4
    B               NXT1
NXT:
    MOV             X6, #32
    ADD             X3, X3, #28

NXT1:
    SUB             X4, X4, #1
    ASR             X4, X4, #2
    SUB             x7, x7, #28














NEON_PROLOGUE:

    MOV             x8, #-32

    dup             v14.4s, w5

    SUB             X1, X1, #28

    LD2             {v8.h, v9.h}[0], [x3], x6
    LD2             {v8.h, v9.h}[1], [x3], x6
    LD2             {v8.h, v9.h}[2], [x3], x6
    LD2             {v8.h, v9.h}[3], [x3], x6

    rev64           v10.4h, v8.4h
    rev64           v11.4h, v9.4h

    LD4             {v0.4h, v1.4h, v2.4h, v3.4h}, [x0], #32

    LD4             {v4.4h, v5.4h, v6.4h, v7.4h}, [x1], x8

    rev64           v0.4h, v0.4h
    rev64           v1.4h, v1.4h
    rev64           v4.4h, v4.4h
    rev64           v5.4h, v5.4h







    uMULL           v30.4s, v2.4h, v9.4h
    uMULL           v28.4s, v4.4h, v9.4h
    uMULL           v26.4s, v2.4h, v8.4h
    uMULL           v24.4s, v4.4h, v8.4h

    ushR            v30.4s, v30.4s, #16
    ushR            v28.4s, v28.4s, #16
    ushR            v26.4s, v26.4s, #16
    ushR            v24.4s, v24.4s, #16

    sMLAL           v30.4s, v3.4h, v9.4h
    sMLAL           v28.4s, v5.4h, v9.4h
    sMLAL           v26.4s, v3.4h, v8.4h
    sMLAL           v24.4s, v5.4h, v8.4h

    ADD             v28.4s, v26.4s , v28.4s
    NEG             v28.4s, v28.4s
    SUB             v30.4s, v30.4s , v24.4s

    uMULL           v22.4s, v0.4h, v11.4h
    uMULL           v20.4s, v6.4h, v11.4h
    uMULL           v18.4s, v0.4h, v10.4h
    uMULL           v16.4s, v6.4h, v10.4h

    ushR            v22.4s, v22.4s, #16
    ushR            v20.4s, v20.4s, #16
    ushR            v18.4s, v18.4s, #16
    ushR            v16.4s, v16.4s, #16

    sMLAL           v22.4s, v1.4h, v11.4h
    LD2             {v8.h, v9.h}[0], [x3], x6

    sMLAL           v20.4s, v7.4h, v11.4h
    LD2             {v8.h, v9.h}[1], [x3], x6

    sMLAL           v18.4s, v1.4h, v10.4h
    LD2             {v8.h, v9.h}[2], [x3], x6

    sMLAL           v16.4s, v7.4h, v10.4h
    LD2             {v8.h, v9.h}[3], [x3], x6

    ADD             v20.4s, v20.4s , v18.4s

    NEG             v20.4s, v20.4s
    rev64           v10.4h, v8.4h
    rev64           v11.4h, v9.4h
    SUB             v22.4s, v16.4s , v22.4s
    LD4             {v0.4h, v1.4h, v2.4h, v3.4h}, [x0], #32



    sshL            v20.4s, v20.4s, v14.4s
    LD4             {v4.4h, v5.4h, v6.4h, v7.4h}, [x1], x8

    rev64           v0.4h, v0.4h
    rev64           v1.4h, v1.4h
    sshL            v22.4s, v22.4s, v14.4s

    rev64           v4.4h, v4.4h
    rev64           v5.4h, v5.4h
    sshL            v18.4s, v30.4s, v14.4s


    sshL            v16.4s, v28.4s, v14.4s








    SUB             X4, X4, #2

CORE_LOOP:
    uMULL           v30.4s, v2.4h, v9.4h
    MOV             v17.16B, v18.16B
    ST2             { v16.4s, v17.4s}, [x2]
    ADD             x2, x2, #32
    uMULL           v28.4s, v4.4h, v9.4h

    uMULL           v26.4s, v2.4h, v8.4h
    MOV             v21.16B, v22.16B
    ST2             { v20.4s, v21.4s}, [x7], x8
    uMULL           v24.4s, v4.4h, v8.4h

    ushR            v30.4s, v30.4s, #16
    ushR            v28.4s, v28.4s, #16
    ushR            v26.4s, v26.4s, #16
    ushR            v24.4s, v24.4s, #16

    sMLAL           v30.4s, v3.4h, v9.4h
    sMLAL           v28.4s, v5.4h, v9.4h
    sMLAL           v26.4s, v3.4h, v8.4h
    sMLAL           v24.4s, v5.4h, v8.4h

    ADD             v28.4s, v26.4s , v28.4s
    NEG             v28.4s, v28.4s
    SUB             v30.4s, v30.4s , v24.4s

    uMULL           v22.4s, v0.4h, v11.4h
    LD2             {v8.h, v9.h}[0], [x3], x6
    uMULL           v20.4s, v6.4h, v11.4h

    uMULL           v18.4s, v0.4h, v10.4h
    LD2             {v8.h, v9.h}[1], [x3], x6
    uMULL           v16.4s, v6.4h, v10.4h

    ushR            v22.4s, v22.4s, #16
    LD2             {v8.h, v9.h}[2], [x3], x6
    ushR            v20.4s, v20.4s, #16


    ushR            v18.4s, v18.4s, #16
    LD2             {v8.h, v9.h}[3], [x3], x6
    ushR            v16.4s, v16.4s, #16

    sMLAL           v22.4s, v1.4h, v11.4h

    sMLAL           v20.4s, v7.4h, v11.4h


    sMLAL           v18.4s, v1.4h, v10.4h


    sMLAL           v16.4s, v7.4h, v10.4h
    LD4             {v0.4h, v1.4h, v2.4h, v3.4h}, [x0], #32
    ADD             v20.4s, v20.4s , v18.4s

    NEG             v20.4s, v20.4s
    rev64           v10.4h, v8.4h
    rev64           v11.4h, v9.4h

    SUB             v22.4s, v16.4s , v22.4s
    LD4             {v4.4h, v5.4h, v6.4h, v7.4h}, [x1], x8
    sshL            v20.4s, v20.4s, v14.4s


    sshL            v22.4s, v22.4s, v14.4s

    rev64           v0.4h, v0.4h
    rev64           v1.4h, v1.4h
    sshL            v18.4s, v30.4s, v14.4s

    rev64           v4.4h, v4.4h
    rev64           v5.4h, v5.4h
    sshL            v16.4s, v28.4s, v14.4s


    SUBS            x4, x4, #1
    BNE             CORE_LOOP






NEON_EPILOGUE:
    uMULL           v30.4s, v2.4h, v9.4h
    MOV             v17.16B, v18.16B
    ST2             { v16.4s, v17.4s}, [x2]
    ADD             x2, x2, #32
    uMULL           v28.4s, v4.4h, v9.4h

    uMULL           v26.4s, v2.4h, v8.4h
    MOV             v21.16B, v22.16B

    ST2             { v20.4s, v21.4s}, [x7], x8
    uMULL           v24.4s, v4.4h, v8.4h

    ushR            v30.4s, v30.4s, #16
    ushR            v28.4s, v28.4s, #16
    ushR            v26.4s, v26.4s, #16
    ushR            v24.4s, v24.4s, #16

    sMLAL           v30.4s, v3.4h, v9.4h
    sMLAL           v28.4s, v5.4h, v9.4h
    sMLAL           v26.4s, v3.4h, v8.4h
    sMLAL           v24.4s, v5.4h, v8.4h

    ADD             v28.4s, v26.4s , v28.4s
    NEG             v28.4s, v28.4s
    SUB             v30.4s, v30.4s , v24.4s

    uMULL           v22.4s, v0.4h, v11.4h
    uMULL           v20.4s, v6.4h, v11.4h
    uMULL           v18.4s, v0.4h, v10.4h
    uMULL           v16.4s, v6.4h, v10.4h

    ushR            v22.4s, v22.4s, #16
    ushR            v20.4s, v20.4s, #16
    ushR            v18.4s, v18.4s, #16
    ushR            v16.4s, v16.4s, #16

    sMLAL           v22.4s, v1.4h, v11.4h
    sMLAL           v20.4s, v7.4h, v11.4h
    sMLAL           v18.4s, v1.4h, v10.4h
    sMLAL           v16.4s, v7.4h, v10.4h

    ADD             v20.4s, v20.4s , v18.4s
    NEG             v20.4s, v20.4s
    SUB             v22.4s, v16.4s , v22.4s


    sshL            v20.4s, v20.4s, v14.4s
    sshL            v22.4s, v22.4s, v14.4s
    sshL            v18.4s, v30.4s, v14.4s
    sshL            v16.4s, v28.4s, v14.4s
    MOV             v17.16B, v18.16B
    ST2             { v16.4s, v17.4s}, [x2]
    ADD             x2, x2, #32
    MOV             v21.16B, v22.16B
    ST2             { v20.4s, v21.4s}, [x7], x8


RESIDUE_NEON:
    MOV             x10, #-16
    movi            v3.2s, #0x00000000
    movi            v4.2s, #0x00000000

    LD2             {v21.2s, v22.2s}, [x0], #16
    MOV             v0.8B, v21.8B
    MOV             v2.8B, v22.8B

    LD1             {v1.s}[0], [x0], #4;
    LD1             {v3.s}[0], [x0], #4;
    LD1             {v1.s}[1], [x0]
    MOV             v21.8B, v0.8B

    UZP1            v0.4h, v21.4h, v1.4h
    UZP2            v1.4h, v21.4h, v1.4h
    MOV             v21.8B, v2.8B
    UZP1            v2.4h, v21.4h, v3.4h
    UZP2            v3.4h, v21.4h, v3.4h

    ADD             x1, x1, #4

    LD1             {v6.s}[0], [x1], #4
    LD1             {v4.s}[1], [x1], #4
    LD1             {v6.s}[1], [x1], #4


    LD2             {v21.2s, v22.2s}, [x1], #16
    MOV             v5.8B, v21.8B
    MOV             v7.8B, v22.8B


    MOV             v21.8B, v4.8B
    UZP1            v4.4h, v21.4h, v5.4h
    UZP2            v5.4h, v21.4h, v5.4h
    MOV             v21.8B, v6.8B
    UZP1            v6.4h, v21.4h, v7.4h
    UZP2            v7.4h, v21.4h, v7.4h
    rev64           v0.4h, v0.4h
    rev64           v1.4h, v1.4h
    rev64           v4.4h, v4.4h
    rev64           v5.4h, v5.4h

    LD2             {v8.h, v9.h}[0], [x3], x6
    LD2             {v8.h, v9.h}[1], [x3], x6
    LD2             {v8.h, v9.h}[2], [x3], x6
    LD2             {v8.h, v9.h}[3], [x3], x6

    rev64           v10.4h, v8.4h
    rev64           v11.4h, v9.4h



    uMULL           v30.4s, v2.4h, v9.4h
    uMULL           v28.4s, v4.4h, v9.4h
    uMULL           v26.4s, v2.4h, v8.4h
    uMULL           v24.4s, v4.4h, v8.4h

    ushR            v30.4s, v30.4s, #16
    ushR            v28.4s, v28.4s, #16
    ushR            v26.4s, v26.4s, #16
    ushR            v24.4s, v24.4s, #16

    sMLAL           v30.4s, v3.4h, v9.4h
    sMLAL           v28.4s, v5.4h, v9.4h
    sMLAL           v26.4s, v3.4h, v8.4h
    sMLAL           v24.4s, v5.4h, v8.4h

    ADD             v28.4s, v26.4s , v28.4s
    NEG             v28.4s, v28.4s
    SUB             v30.4s, v30.4s , v24.4s

    uMULL           v22.4s, v0.4h, v11.4h
    uMULL           v20.4s, v6.4h, v11.4h
    uMULL           v18.4s, v0.4h, v10.4h
    uMULL           v16.4s, v6.4h, v10.4h

    ushR            v22.4s, v22.4s, #16
    ushR            v20.4s, v20.4s, #16
    ushR            v18.4s, v18.4s, #16
    ushR            v16.4s, v16.4s, #16

    sMLAL           v22.4s, v1.4h, v11.4h
    sMLAL           v20.4s, v7.4h, v11.4h
    sMLAL           v18.4s, v1.4h, v10.4h
    sMLAL           v16.4s, v7.4h, v10.4h

    ADD             v20.4s, v20.4s , v18.4s
    NEG             v20.4s, v20.4s
    SUB             v22.4s, v16.4s , v22.4s



    sshL            v20.4s, v20.4s, v14.4s
    sshL            v22.4s, v22.4s, v14.4s
    sshL            v18.4s, v30.4s, v14.4s
    sshL            v16.4s, v28.4s, v14.4s
    MOV             v21.16B, v22.16B
    ST2             { v20.4s, v21.4s}, [x7]
    mov             v17.16B, v18.16B
    ST2             {v16.2s, v17.2s}, [x2]
    ADD             x2, x2, #16

    ST2             {v16.s, v17.s}[2], [x2]
    ADD             x2, x2, #8






END1:
    pop_v_regs
    ret



