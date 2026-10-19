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
.global ixheaacd_sbr_qmfsyn64_winadd

ixheaacd_sbr_qmfsyn64_winadd:

    push_v_regs



    MOV             w7, #0x8000
    LD1             {v0.4h}, [x0], #8
    MOV             x12, x2

    dup             v30.4s, w7
    LD1             {v1.4h}, [x2], #8
    dup             v22.4s, w4

    MOV             x10, x0
    MOV             x11, x2
    ADD             x0, x0, #504
    ADD             x2, x2, #248

    NEG             v28.4s, v22.4s
    sshL            v20.4s, v30.4s, v28.4s
    MOV             x6, #64
    LSL             x6, x6, #1
    ADD             x12, x12, x6
    MOV             x7, #128
    LSL             x9, x7, #1
    ADD             x1, x1, x9
    MOV             x6, #16
    MOV             x7, #128
    LSL             x9, x7, #1
    MOV             x7, #256
    LSL             x8, x7, #1

    LSL             x5, x5, #1
    LD1             {v2.4h}, [x0], x8
    mov             v26.16b, v20.16b


    sMLAL           v26.4s, v0.4h, v1.4h
    LD1             {v3.4h}, [x2], x9

    LD1             {v4.4h}, [x0], x8
    sMLAL           v26.4s, v2.4h, v3.4h

    LD1             {v5.4h}, [x2], x9

    LD1             {v6.4h}, [x0], x8
    sMLAL           v26.4s, v5.4h, v4.4h

    LD1             {v7.4h}, [x2], x9

    LD1             {v8.4h}, [x0], x8
    sMLAL           v26.4s, v7.4h, v6.4h

    LD1             {v9.4h}, [x2], x9
    MOV             x0, x10


    MOV             x2, x11
    LD1             {v10.4h}, [x1], #8
    sMLAL           v26.4s, v9.4h, v8.4h

    MOV             x10, x1
    LD1             {v11.4h}, [x12], #8
    ADD             x1, x1, #504



    MOV             x11, x12
    LD1             {v12.4h}, [x1], x8
    ADD             x12, x12, #248

    sMLAL           v26.4s, v10.4h, v11.4h
    LD1             {v13.4h}, [x12], x9

    LD1             {v14.4h}, [x1], x8
    sMLAL           v26.4s, v12.4h, v13.4h

    LD1             {v15.4h}, [x12], x9

    LD1             {v16.4h}, [x1], x8
    sMLAL           v26.4s, v15.4h, v14.4h

    LD1             {v17.4h}, [x12], x9

    LD1             {v18.4h}, [x1], x8
    sMLAL           v26.4s, v17.4h, v16.4h

    LD1             {v19.4h}, [x12], x9

    sMLAL           v26.4s, v19.4h, v18.4h
    LD1             {v0.4h}, [x0], #8
    MOV             x12, x11

    MOV             x1, x10
    LD1             {v1.4h}, [x2], #8
    MOV             x10, x0

    sQshL           v26.4s, v26.4s, v22.4s

    ADD             x0, x0, #504

    MOV             x11, x2
    LD1             {v2.4h}, [x0], x8
    ADD             x2, x2, #248

    sshR            v28.4s, v26.4s, #16
    LD1             {v3.4h}, [x2], x9


    UZP2            v29.8h, v28.8h, v28.8h
    UZP1            v28.8h, v28.8h, v28.8h
    mov             v26.16b, v20.16b




    LD1             {v4.4h}, [x0], x8
    LD1             {v5.4h}, [x2], x9

    LD1             {v6.4h}, [x0], x8
    LD1             {v7.4h}, [x2], x9

    LD1             {v8.4h}, [x0], x8
    LD1             {v9.4h}, [x2], x9
    MOV             x0, x10


    MOV             x2, x11
    LD1             {v10.4h}, [x1], #8

    MOV             x10, x1
    LD1             {v11.4h}, [x12], #8
    ADD             x1, x1, #504


    MOV             x11, x12
    LD1             {v12.4h}, [x1], x8
    ADD             x12, x12, #248


    LD1             {v13.4h}, [x12], x9

    LD1             {v14.4h}, [x1], x8
    LD1             {v15.4h}, [x12], x9

    LD1             {v16.4h}, [x1], x8
    LD1             {v17.4h}, [x12], x9

    LD1             {v18.4h}, [x1], x8
    SUB             x6, x6, #2
    LD1             {v19.4h}, [x12], x9
    MOV             x1, x10

    MOV             x12, x11

LOOP_1:

    sMLAL           v26.4s, v0.4h, v1.4h
    ST1             {v28.h}[0], [x3], x5

    sMLAL           v26.4s, v2.4h, v3.4h
    LD1             {v0.4h}, [x0], #8
    sMLAL           v26.4s, v5.4h, v4.4h

    sMLAL           v26.4s, v7.4h, v6.4h
    ST1             {v28.h}[1], [x3], x5


    MOV             x10, x0
    LD1             {v1.4h}, [x2], #8
    ADD             x0, x0, #504

    sMLAL           v26.4s, v9.4h, v8.4h
    ST1             {v28.h}[2], [x3], x5

    sMLAL           v26.4s, v10.4h, v11.4h
    ST1             {v28.h}[3], [x3], x5

    MOV             x11, x2
    LD1             {v2.4h}, [x0], x8
    ADD             x2, x2, #248

    sMLAL           v26.4s, v12.4h, v13.4h
    LD1             {v3.4h}, [x2], x9
    sMLAL           v26.4s, v15.4h, v14.4h

    sMLAL           v26.4s, v17.4h, v16.4h
    LD1             {v4.4h}, [x0], x8
    sMLAL           v26.4s, v19.4h, v18.4h

    LD1             {v5.4h}, [x2], x9

    LD1             {v6.4h}, [x0], x8
    sQshL           v26.4s, v26.4s, v22.4s

    sshR            v28.4s, v26.4s, #16
    LD1             {v7.4h}, [x2], x9
    mov             v26.16b, v20.16b


    UZP2            v29.8h, v28.8h, v28.8h
    UZP1            v28.8h, v28.8h, v28.8h
    sMLAL           v26.4s, v0.4h, v1.4h

    sMLAL           v26.4s, v2.4h, v3.4h
    LD1             {v8.4h}, [x0], x8
    sMLAL           v26.4s, v5.4h, v4.4h

    sMLAL           v26.4s, v7.4h, v6.4h
    LD1             {v9.4h}, [x2], x9


    LD1             {v10.4h}, [x1], #8
    sMLAL           v26.4s, v9.4h, v8.4h

    MOV             x2, x11
    LD1             {v11.4h}, [x12], #8
    MOV             x0, x10

    MOV             x10, x1

    ADD             x1, x1, #504

    MOV             x11, x12
    LD1             {v12.4h}, [x1], x8
    ADD             x12, x12, #248

    LD1             {v13.4h}, [x12], x9
    sMLAL           v26.4s, v10.4h, v11.4h

    LD1             {v14.4h}, [x1], x8
    sMLAL           v26.4s, v12.4h, v13.4h

    LD1             {v15.4h}, [x12], x9

    LD1             {v16.4h}, [x1], x8
    sMLAL           v26.4s, v15.4h, v14.4h

    LD1             {v17.4h}, [x12], x9

    LD1             {v18.4h}, [x1], x8
    sMLAL           v26.4s, v17.4h, v16.4h

    LD1             {v19.4h}, [x12], x9
    MOV             x1, x10

    sMLAL           v26.4s, v19.4h, v18.4h
    ST1             {v28.h}[0], [x3], x5

    MOV             x12, x11
    LD1             {v0.4h}, [x0], #8

    LD1             {v1.4h}, [x2], #8
    sQshL           v26.4s, v26.4s, v22.4s


    ST1             {v28.h}[1], [x3], x5
    MOV             x10, x0

    ST1             {v28.h}[2], [x3], x5
    ADD             x0, x0, #504

    ST1             {v28.h}[3], [x3], x5
    MOV             x11, x2

    sshR            v28.4s, v26.4s, #16
    LD1             {v2.4h}, [x0], x8
    ADD             x2, x2, #248

    LD1             {v3.4h}, [x2], x9
    LD1             {v4.4h}, [x0], x8
    LD1             {v5.4h}, [x2], x9
    LD1             {v6.4h}, [x0], x8
    LD1             {v7.4h}, [x2], x9
    LD1             {v8.4h}, [x0], x8
    LD1             {v9.4h}, [x2], x9

    UZP2            v29.8h, v28.8h, v28.8h
    UZP1            v28.8h, v28.8h, v28.8h
    mov             v26.16b, v20.16b




    MOV             x0, x10
    LD1             {v10.4h}, [x1], #8
    MOV             x2, x11

    MOV             x10, x1
    LD1             {v11.4h}, [x12], #8
    ADD             x1, x1, #504


    MOV             x11, x12
    LD1             {v12.4h}, [x1], x8
    ADD             x12, x12, #248


    LD1             {v13.4h}, [x12], x9

    LD1             {v14.4h}, [x1], x8
    LD1             {v15.4h}, [x12], x9

    LD1             {v16.4h}, [x1], x8
    LD1             {v17.4h}, [x12], x9

    SUBS            x6, x6, #2
    LD1             {v18.4h}, [x1], x8

    MOV             x1, x10
    LD1             {v19.4h}, [x12], x9

    MOV             x12, x11


    BGT             LOOP_1

    sMLAL           v26.4s, v0.4h, v1.4h
    ST1             {v28.h}[0], [x3], x5
    sMLAL           v26.4s, v2.4h, v3.4h

    sMLAL           v26.4s, v5.4h, v4.4h
    ST1             {v28.h}[1], [x3], x5
    sMLAL           v26.4s, v7.4h, v6.4h

    sMLAL           v26.4s, v9.4h, v8.4h
    ST1             {v28.h}[2], [x3], x5
    sMLAL           v26.4s, v10.4h, v11.4h

    sMLAL           v26.4s, v12.4h, v13.4h
    ST1             {v28.h}[3], [x3], x5
    sMLAL           v26.4s, v15.4h, v14.4h



    sMLAL           v26.4s, v17.4h, v16.4h

    sMLAL           v26.4s, v19.4h, v18.4h

    sQshL           v26.4s, v26.4s, v22.4s

    sshR            v28.4s, v26.4s, #16

    UZP2            v29.8h, v28.8h, v28.8h
    UZP1            v28.8h, v28.8h, v28.8h


    ST1             {v28.h}[0], [x3], x5
    ST1             {v28.h}[1], [x3], x5
    ST1             {v28.h}[2], [x3], x5
    ST1             {v28.h}[3], [x3], x5


    pop_v_regs
    ret

