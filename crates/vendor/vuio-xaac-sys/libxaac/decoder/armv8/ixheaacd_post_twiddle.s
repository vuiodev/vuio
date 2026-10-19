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
    stp             x21, x22, [sp, #-16]!
    stp             x23, x24, [sp, #-16]!
.endm
.macro pop_v_regs
    ldp             x23, x24, [sp], #16
    ldp             x21, x22, [sp], #16
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
.global ixheaacd_post_twiddle_armv8
ixheaacd_post_twiddle_armv8:


    push_v_regs

ARM_PROLOGUE:
    CMP             w3, #0x400
    MOV             x21, #7500
    ADD             x2, x2, x21
    BLT             NEXT
    MOV             w4, #50
    MOV             w5, #-50
    MOV             x6, #4
    dup             v10.4h, w4
    B               NEXT1

NEXT:
    MOV             w4, #0x192
    MOV             w5, #0xfe6e
    MOV             x6, #32
    dup             v10.4h, w4

NEXT1:
    LDR             w9, [x2]
    LSL             W22, W9, #16
    AND             W21, W9, #0xFFFF0000

    LDR             w7, [x1], #4
    LDR             w8, [x1], #4

    ADD             x2, x2, x6


    SMULL           X11, w8, w21
    ASR             X11, x11, #32
    SMULL           X10, w8, w22
    ASR             X10, x10, #32
    SMULL           X12, w7, w21
    ASR             X12, x12, #32
    SMULL           X23, w7, w22
    ASR             X23, x23, #32
    ADD             w8, w11, w23


    SUB             w10, w10, w12

    MVN             w8, w8
    ADD             w8, w8, #1



    LSL             w21, w5, #16
    LSL             w22, w4, #16
    SMULL           X23, w10, w21
    ASR             X23, x23, #32
    ADD             w9, w8, w23
    SMULL           X23, w8, w22
    ASR             X23, x23, #32
    ADD             w11, w10, w23

    LSL             x7, x3, #2
    ADD             x7, x0, x7
    SUB             x7, x7, #4

    STR             w11, [x7], #-4

    STR             w9, [x0], #4

    LSL             x5, x3, #2
    ADD             x5, x1, x5
    SUB             x5, x5, #40


    SUB             w3, w3, #1
    ASR             w3, w3, #4


    SUB             x7, x7, #28












    MOV             x8, #-32

NEON_PROLOGUE:

    LD4             {v0.4h, v1.4h, v2.4h, v3.4h}, [x5], x8

    LD4             {v4.4h, v5.4h, v6.4h, v7.4h}, [x1], #32
    LD2             {v8.h, v9.h}[0], [x2], x6
    LD2             {v8.h, v9.h}[1], [x2], x6
    LD2             {v8.h, v9.h}[2], [x2], x6
    LD2             {v8.h, v9.h}[3], [x2], x6

    rev64           v12.4h, v8.4h
    rev64           v13.4h, v9.4h

    uMULL           v30.4s, v2.4h, v13.4h
    uMULL           v28.4s, v0.4h, v13.4h
    uMULL           v26.4s, v2.4h, v12.4h
    uMULL           v24.4s, v0.4h, v12.4h

    ushR            v30.4s, v30.4s, #16
    ushR            v28.4s, v28.4s, #16
    ushR            v26.4s, v26.4s, #16
    ushR            v24.4s, v24.4s, #16

    sMLAL           v30.4s, v3.4h, v13.4h
    sMLAL           v28.4s, v1.4h, v13.4h
    sMLAL           v26.4s, v3.4h, v12.4h
    sMLAL           v24.4s, v1.4h, v12.4h

    uMULL           v22.4s, v6.4h, v9.4h
    uMULL           v20.4s, v4.4h, v9.4h

    ADD             v28.4s, v28.4s , v26.4s
    SUB             v30.4s, v30.4s , v24.4s
    NEG             v28.4s, v28.4s

    uMULL           v18.4s, v6.4h, v8.4h
    uMULL           v16.4s, v4.4h, v8.4h

    mov             v31.8b, v30.8b
    mov             v27.D[0], v30.D[1]
    ushR            v22.4s, v22.4s, #16

    mov             v24.8b, v28.8b
    mov             v25.D[0], v28.D[1]
    ushR            v20.4s, v20.4s, #16


    UZP1            v26.4h, v31.4h, v27.4h
    UZP2            v27.4h, v31.4h, v27.4h
    ushR            v18.4s, v18.4s, #16


    mov             v31.8B , v24.8B
    UZP1            v24.4h, v31.4h, v25.4h
    UZP2            v25.4h, v31.4h, v25.4h
    ushR            v16.4s, v16.4s, #16


    sMLAL           v22.4s, v7.4h, v9.4h
    sMLAL           v20.4s, v5.4h, v9.4h
    sMLAL           v18.4s, v7.4h, v8.4h
    sMLAL           v16.4s, v5.4h, v8.4h

    LD2             {v8.h, v9.h}[0], [x2], x6
    uMULL           v0.4s, v26.4h, v10.4h

    LD2             {v8.h, v9.h}[1], [x2], x6
    uMULL           v2.4s, v24.4h, v10.4h


    LD2             {v8.h, v9.h}[2], [x2], x6
    ADD             v22.4s, v22.4s , v16.4s

    LD2             {v8.h, v9.h}[3], [x2], x6
    SUB             v20.4s, v18.4s , v20.4s

    rev64           v12.4h, v8.4h
    rev64           v13.4h, v9.4h
    NEG             v22.4s, v22.4s


    mov             v18.8b, v22.8b
    mov             v19.D[0], v22.D[1]
    ushR            v0.4s, v0.4s, #16

    mov             v16.16b, v20.16b
    mov             v17.D[0], v20.D[1]
    ushR            v2.4s, v2.4s, #16


    MOV             v31.8b, v18.8b
    UZP1            v18.4h, v31.4h, v19.4h
    UZP2            v19.4h, v31.4h, v19.4h
    sMLAL           v0.4s, v27.4h, v10.4h


    MOV             v31.8b, v16.8b
    UZP1            v16.4h, v31.4h, v17.4h
    UZP2            v17.4h, v31.4h, v17.4h
    sMLAL           v2.4s, v25.4h, v10.4h

    uMULL           v4.4s, v18.4h, v10.4h
    uMULL           v6.4s, v16.4h, v10.4h

    NEG             v0.4s, v0.4s
    ADD             v14.4s, v30.4s , v2.4s
    ADD             v26.4s, v28.4s , v0.4s

    rev64           v14.4s, v14.4s
    ushR            v4.4s, v4.4s, #16

    swp             v14.D[0], v14.D[1]
    ushR            v6.4s, v6.4s, #16

    sMLAL           v4.4s, v19.4h, v10.4h
    LD4             {v0.4h, v1.4h, v2.4h, v3.4h}, [x5], x8
    sMLAL           v6.4s, v17.4h, v10.4h


    SUB             x3, x3, #2

    ADD             v24.4s, v20.4s , v4.4s

    rev64           v24.4s, v24.4s
    NEG             v16.4s, v6.4s

    LD4             {v4.4h, v5.4h, v6.4h, v7.4h}, [x1], #32

    swp             v24.D[0], v24.D[1]
    ADD             v16.4s, v22.4s , v16.4s



CORE_LOOP:
    uMULL           v30.4s, v2.4h, v13.4h
    MOV             v25.16B, v24.16B
    ST2             { v25.4s, v26.4s}, [x7], x8
    uMULL           v28.4s, v0.4h, v13.4h

    uMULL           v26.4s, v2.4h, v12.4h
    MOV             v15.16B, v14.16B
    ST2             { v15.4s, v16.4s}, [x0], #32
    uMULL           v24.4s, v0.4h, v12.4h

    ushR            v30.4s, v30.4s, #16
    ushR            v28.4s, v28.4s, #16
    ushR            v26.4s, v26.4s, #16
    ushR            v24.4s, v24.4s, #16

    sMLAL           v30.4s, v3.4h, v13.4h
    sMLAL           v28.4s, v1.4h, v13.4h
    sMLAL           v26.4s, v3.4h, v12.4h
    sMLAL           v24.4s, v1.4h, v12.4h

    uMULL           v22.4s, v6.4h, v9.4h
    uMULL           v20.4s, v4.4h, v9.4h


    ADD             v28.4s, v28.4s , v26.4s
    SUB             v30.4s, v30.4s , v24.4s
    NEG             v28.4s, v28.4s

    uMULL           v18.4s, v6.4h, v8.4h
    uMULL           v16.4s, v4.4h, v8.4h


    mov             v26.8b, v30.8b
    mov             v27.D[0], v30.D[1]
    ushR            v22.4s, v22.4s, #16


    mov             v24.8b, v28.8b
    mov             v25.D[0], v28.D[1]
    ushR            v20.4s, v20.4s, #16


    MOV             v31.8b, v26.8b
    UZP1            v26.4h, v31.4h, v27.4h
    UZP2            v27.4h, v31.4h, v27.4h
    ushR            v18.4s, v18.4s, #16


    MOV             v31.8b, v24.8b
    UZP1            v24.4h, v31.4h, v25.4h
    UZP2            v25.4h, v31.4h, v25.4h
    ushR            v16.4s, v16.4s, #16


    sMLAL           v22.4s, v7.4h, v9.4h
    sMLAL           v20.4s, v5.4h, v9.4h
    sMLAL           v18.4s, v7.4h, v8.4h
    sMLAL           v16.4s, v5.4h, v8.4h

    LD2             {v8.h, v9.h}[0], [x2], x6
    uMULL           v0.4s, v26.4h, v10.4h

    LD2             {v8.h, v9.h}[1], [x2], x6
    uMULL           v2.4s, v24.4h, v10.4h

    LD2             {v8.h, v9.h}[2], [x2], x6
    ADD             v22.4s, v22.4s , v16.4s

    LD2             {v8.h, v9.h}[3], [x2], x6
    SUB             v20.4s, v18.4s , v20.4s

    rev64           v12.4h, v8.4h
    rev64           v13.4h, v9.4h
    NEG             v22.4s, v22.4s

    mov             v18.8b, v22.8b
    mov             v19.D[0], v22.D[1]
    ushR            v0.4s, v0.4s, #16

    mov             v16.8b, v20.8b
    mov             v17.D[0], v20.D[1]
    ushR            v2.4s, v2.4s, #16


    MOV             v31.8b, v18.8b
    UZP1            v18.4h, v31.4h, v19.4h
    UZP2            v19.4h, v31.4h, v19.4h
    sMLAL           v0.4s, v27.4h, v10.4h


    MOV             v31.8b, v16.8b
    UZP1            v16.4h, v31.4h, v17.4h
    UZP2            v17.4h, v31.4h, v17.4h
    sMLAL           v2.4s, v25.4h, v10.4h

    uMULL           v4.4s, v18.4h, v10.4h
    uMULL           v6.4s, v16.4h, v10.4h

    NEG             v0.4s, v0.4s
    ADD             v14.4s, v30.4s , v2.4s
    ADD             v26.4s, v28.4s , v0.4s

    rev64           v14.4s, v14.4s
    ushR            v4.4s, v4.4s, #16

    swp             v14.D[0], v14.D[1]
    ushR            v6.4s, v6.4s, #16

    sMLAL           v4.4s, v19.4h, v10.4h
    LD4             {v0.4h, v1.4h, v2.4h, v3.4h}, [x5], x8
    sMLAL           v6.4s, v17.4h, v10.4h





    ADD             v24.4s, v20.4s , v4.4s

    rev64           v24.4s, v24.4s
    NEG             v16.4s, v6.4s

    LD4             {v4.4h, v5.4h, v6.4h, v7.4h}, [x1], #32

    swp             v24.D[0], v24.D[1]
    ADD             v16.4s, v22.4s , v16.4s

    SUBS            x3, x3, #1

    BNE             CORE_LOOP




NEON_EPILOGUE:
    uMULL           v30.4s, v2.4h, v13.4h
    MOV             v25.16B, v24.16B
    ST2             { v25.4s, v26.4s}, [x7], x8
    uMULL           v28.4s, v0.4h, v13.4h

    uMULL           v26.4s, v2.4h, v12.4h
    MOV             v15.16B, v14.16B
    ST2             { v15.4s, v16.4s}, [x0], #32
    uMULL           v24.4s, v0.4h, v12.4h



    ushR            v30.4s, v30.4s, #16
    ushR            v28.4s, v28.4s, #16
    ushR            v26.4s, v26.4s, #16
    ushR            v24.4s, v24.4s, #16

    sMLAL           v30.4s, v3.4h, v13.4h
    sMLAL           v28.4s, v1.4h, v13.4h
    sMLAL           v26.4s, v3.4h, v12.4h
    sMLAL           v24.4s, v1.4h, v12.4h


    uMULL           v22.4s, v6.4h, v9.4h
    uMULL           v20.4s, v4.4h, v9.4h


    ADD             v28.4s, v28.4s , v26.4s
    SUB             v30.4s, v30.4s , v24.4s
    NEG             v28.4s, v28.4s

    uMULL           v18.4s, v6.4h, v8.4h
    uMULL           v16.4s, v4.4h, v8.4h


    mov             v26.8b, v30.8b
    mov             v27.D[0], v30.D[1]
    ushR            v22.4s, v22.4s, #16

    mov             v24.16b, v28.16b
    mov             v25.D[0], v28.D[1]
    ushR            v20.4s, v20.4s, #16


    mov             v31.8b, v26.8b
    UZP1            v26.4h, v31.4h, v27.4h
    UZP2            v27.4h, v31.4h, v27.4h
    ushR            v18.4s, v18.4s, #16


    mov             v31.8b, v24.8b
    UZP1            v24.4h, v31.4h, v25.4h
    UZP2            v25.4h, v31.4h, v25.4h
    ushR            v16.4s, v16.4s, #16


    sMLAL           v22.4s, v7.4h, v9.4h
    sMLAL           v20.4s, v5.4h, v9.4h
    sMLAL           v18.4s, v7.4h, v8.4h
    sMLAL           v16.4s, v5.4h, v8.4h


    uMULL           v0.4s, v26.4h, v10.4h


    uMULL           v2.4s, v24.4h, v10.4h


    ADD             v22.4s, v22.4s , v16.4s


    SUB             v20.4s, v18.4s , v20.4s


    NEG             v22.4s, v22.4s


    mov             v18.16b, v22.16b
    ushR            v0.4s, v0.4s, #16

    mov             v16.16b, v20.16b
    ushR            v2.4s, v2.4s, #16


    mov             v31.16b, v18.16b
    mov             v19.d[0], v31.d[1]
    UZP1            v18.4h, v31.4h, v19.4h
    UZP2            v19.4h, v31.4h, v19.4h
    sMLAL           v0.4s, v27.4h, v10.4h


    mov             v31.16b, v16.16b
    mov             v17.d[0], v31.d[1]
    UZP1            v16.4h, v31.4h, v17.4h
    UZP2            v17.4h, v31.4h, v17.4h
    sMLAL           v2.4s, v25.4h, v10.4h

    uMULL           v4.4s, v18.4h, v10.4h
    uMULL           v6.4s, v16.4h, v10.4h

    NEG             v0.4s, v0.4s
    ADD             v14.4s, v30.4s , v2.4s
    ADD             v26.4s, v28.4s , v0.4s

    rev64           v14.4s, v14.4s
    ushR            v4.4s, v4.4s, #16

    swp             v14.D[0], v14.D[1]
    ushR            v6.4s, v6.4s, #16

    sMLAL           v4.4s, v19.4h, v10.4h

    sMLAL           v6.4s, v17.4h, v10.4h




    ADD             v24.4s, v20.4s , v4.4s

    rev64           v24.4s, v24.4s
    NEG             v16.4s, v6.4s



    swp             v24.D[0], v24.D[1]
    ADD             v16.4s, v22.4s , v16.4s

    MOV             v25.16B, v24.16B
    MOV             v15.16B, v14.16B
    ST2             { v15.4s, v16.4s}, [x0], #32
    ST2             { v25.4s, v26.4s}, [x7], x8




    LD4             {v0.4h, v1.4h, v2.4h, v3.4h}, [x5], x8

    movi            v6.2s, #0x00000000
    movi            v7.2s, #0x00000000

    LD2             {v4.2s, v5.2s}, [x1], #16
    LD2             {v6.s, v7.s}[0], [x1]

    LD2             {v8.h, v9.h}[0], [x2], x6
    LD2             {v8.h, v9.h}[1], [x2], x6
    LD2             {v8.h, v9.h}[2], [x2], x6
    LD2             {v8.h, v9.h}[3], [x2], x6

    rev64           v12.8h, v8.8h
    rev64           v13.8h, v9.8h
    swp             v5.D[0], v6.D[0]


    MOV             v30.8B, V4.8B
    UZP1            v4.4h, v30.4h, v5.4h
    UZP2            v5.4h, v30.4h, v5.4h
    MOV             v30.8B, V6.8B
    UZP1            v6.4h, v30.4h, v7.4h
    UZP2            v7.4h, v30.4h, v7.4h
    uMULL           v30.4s, v2.4h, v13.4h
    uMULL           v28.4s, v0.4h, v13.4h

    uMULL           v26.4s, v2.4h, v12.4h
    uMULL           v24.4s, v0.4h, v12.4h

    ushR            v30.4s, v30.4s, #16
    ushR            v28.4s, v28.4s, #16
    ushR            v26.4s, v26.4s, #16
    ushR            v24.4s, v24.4s, #16

    sMLAL           v30.4s, v3.4h, v13.4h
    sMLAL           v28.4s, v1.4h, v13.4h
    sMLAL           v26.4s, v3.4h, v12.4h
    sMLAL           v24.4s, v1.4h, v12.4h

    uMULL           v22.4s, v6.4h, v9.4h
    uMULL           v20.4s, v4.4h, v9.4h


    ADD             v28.4s, v28.4s , v26.4s
    SUB             v30.4s, v30.4s , v24.4s
    NEG             v28.4s, v28.4s

    uMULL           v18.4s, v6.4h, v8.4h
    uMULL           v16.4s, v4.4h, v8.4h

    mov             v26.8b, v30.8b
    mov             v27.D[0], v30.D[1]
    ushR            v22.4s, v22.4s, #16

    mov             v24.16b, v28.16b
    mov             v25.D[0], v28.D[1]
    ushR            v20.4s, v20.4s, #16


    MOV             v31.8B, V26.8B
    UZP1            v26.4h, v31.4h, v27.4h
    UZP2            v27.4h, v31.4h, v27.4h
    ushr            v18.4s, v18.4s, #16

    MOV             v31.8B, V24.8B
    UZP1            v24.4h, v31.4h, v25.4h
    UZP2            v25.4h, v31.4h, v25.4h
    ushR            v16.4s, v16.4s, #16

    sMLAL           v22.4s, v7.4h, v9.4h
    sMLAL           v20.4s, v5.4h, v9.4h
    sMLAL           v18.4s, v7.4h, v8.4h
    sMLAL           v16.4s, v5.4h, v8.4h


    uMULL           v0.4s, v26.4h, v10.4h


    uMULL           v2.4s, v24.4h, v10.4h

    ADD             v22.4s, v22.4s , v16.4s


    SUB             v20.4s, v18.4s , v20.4s


    NEG             v22.4s, v22.4s


    mov             v18.8B, v22.8B
    mov             v19.D[0], v22.D[1]
    ushR            v0.4s, v0.4s, #16

    mov             v16.16b, v20.16b
    mov             v17.D[0], v20.D[1]
    ushR            v2.4s, v2.4s, #16


    MOV             v31.8B, V18.8B
    UZP1            v18.4h, v31.4h, v19.4h
    UZP2            v19.4h, v31.4h, v19.4h
    sMLAL           v0.4s, v27.4h, v10.4h


    MOV             v31.8B, V16.8B
    UZP1            v16.4h, v31.4h, v17.4h
    UZP2            v17.4h, v31.4h, v17.4h
    sMLAL           v2.4s, v25.4h, v10.4h

    uMULL           v4.4s, v18.4h, v10.4h
    uMULL           v6.4s, v16.4h, v10.4h

    NEG             v0.4s, v0.4s
    ADD             v14.4s, v30.4s , v2.4s
    ADD             v26.4s, v28.4s , v0.4s

    rev64           v14.4s, v14.4s
    ushR            v4.4s, v4.4s, #16

    swp             v14.D[0], v14.D[1]
    ushR            v6.4s, v6.4s, #16

    sMLAL           v4.4s, v19.4h, v10.4h

    sMLAL           v6.4s, v17.4h, v10.4h




    ADD             v24.4s, v20.4s , v4.4s

    rev64           v24.4s, v24.4s
    NEG             v16.4s, v6.4s

    swp             v24.D[0], v24.D[1]
    ADD             v16.4s, v22.4s , v16.4s


    MOV             v15.16B, v14.16B
    ST2             {v15.2s, v16.2s}, [x0], #16

    ST2             {v15.s, v16.s}[2], [x0], #8

    ST1             {v15.s}[3], [x0]

    ADD             x7, x7, #4

    ST1             {v26.s}[0], [x7], #4
    MOV             v25.16B, v24.16B
    ST2             {v25.s, v26.s}[1], [x7], #8
    MOV             v27.D[0], V26.d[1]
    mov             v26.d[0], v25.d[1]
    ST2             {v26.2s, v27.2s}, [x7]






    pop_v_regs
    ret












