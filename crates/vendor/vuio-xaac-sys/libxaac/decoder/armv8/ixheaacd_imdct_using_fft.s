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

.macro swp reg1, reg2
    MOv             x16, \reg1
    MOv             \reg1, \reg2
    MOv             \reg2, x16
.endm
.text
.p2align 2
.global ixheaacd_imdct_using_fft_armv8
ixheaacd_imdct_using_fft_armv8:
    push_v_regs

    MOV             X29, #11600
    ADD             X4, X0, X29
    MOV             X29, #11856
    ADD             X5, X0, X29
    MOV             X29, #11920
    ADD             X6, X0, X29
    MOV             X29, #11936
    ADD             X7, X0, X29

COND_1: CMP         X1, #0x400
    BNE             COND_2
    MOv             X8, #4
    B               RADIX_4_FIRST_START


COND_2: CMP         X1, #0x200
    BNE             COND_3
    MOv             X8, #3
    MOv             X4, X5
    B               RADIX_8_FIRST_START

COND_3: CMP         X1, #0x100
    BNE             COND_4
    MOv             X8, #3
    MOv             X4, X5
    B               RADIX_4_FIRST_START

COND_4: CMP         X1, #0x80
    BNE             COND_5
    MOv             X8, #2
    MOv             X4, X6
    B               RADIX_8_FIRST_START

COND_5: CMP         X1, #0x40
    BNE             COND_6
    MOv             X8, #2
    MOv             X4, X6
    B               RADIX_4_FIRST_START
COND_6:
    MOv             X8, #1
    MOv             X4, X7



RADIX_8_FIRST_START:
    LSR             W9 , W1, #5
    LSL             W1, W1, #1

RADIX_8_FIRST_LOOP:

    MOv             X5 , X2
    MOv             X6 , X2
    MOv             X7 , X2
    MOv             X11 , X2






















    LDRB            W12, [X4]
    ADD             X5, X5, X12, LSL #3
    LD2             {v0.S, v1.S}[0], [X5], X1
    ADD             X5, X5, X1
    LD2             {v4.S, v5.S}[0], [X5], X1
    SUB             X5, X5, X1, LSL #1
    LD2             {v2.S, v3.S}[0], [X5], X1
    ADD             X5, X5, X1
    LD2             {v6.S, v7.S}[0], [X5], X1
    SUB             X5, X5, X1, LSL #2

    LDRB            W12, [X4, #1]
    ADD             X6, X6, X12, LSL #3
    LD2             {v0.S, v1.S}[1], [X6] , X1
    ADD             X6, X6, X1
    LD2             {v4.S, v5.S}[1], [X6] , X1
    SUB             X6, X6, X1, LSL #1
    LD2             {v2.S, v3.S}[1], [X6] , X1
    ADD             X6, X6, X1
    LD2             {v6.S, v7.S}[1], [X6], X1
    SUB             X6, X6, X1, LSL #2


    LDRB            W12, [X4, #2]
    ADD             X7, X7, X12, LSL #3
    LD2             {v0.S, v1.S}[2], [X7] , X1
    ADD             X7, X7, X1
    LD2             {v4.S, v5.S}[2], [X7] , X1
    SUB             X7, X7, X1, LSL #1

    LDRB            W12, [X4, #3]
    ADD             X11, X11, X12, LSL #3
    LD2             {v0.S, v1.S}[3], [X11] , X1
    ADD             X11, X11, X1
    LD2             {v4.S, v5.S}[3], [X11] , X1
    SUB             X11, X11, X1, LSL #1


    ADD             v8.4S, v0.4S, v4.4S
    LD2             {v2.S, v3.S}[2], [X7] , X1
    ADD             X7, X7, X1


    SUB             v9.4S, v0.4S, v4.4S
    LD2             {v6.S, v7.S}[2], [X7], X1
    SUB             X7, X7, X1, LSL #2


    ADD             v0.4S, v1.4S, v5.4S
    LD2             {v2.S, v3.S}[3], [X11] , X1
    ADD             X11, X11, X1

    SUB             v4.4S, v1.4S, v5.4S
    LD2             {v6.S, v7.S}[3], [X11], X1
    SUB             X11, X11, X1, LSL #2

    ADD             X4, X4, #4

    ADD             X5, X5, X1, LSR #1
    ADD             X6, X6, X1, LSR #1
    ADD             X7, X7, X1, LSR #1
    ADD             X11, X11, X1, LSR #1


    ADD             v1.4S, v2.4S, v6.4S
    LD2             {v14.S, v15.S}[0], [X5] , X1


    SUB             v5.4S, v2.4S, v6.4S
    LD2             {v10.S, v11.S}[0], [X5] , X1


    ADD             v2.4S, v3.4S, v7.4S
    LD2             {v12.S, v13.S}[0], [X5] , X1


    SUB             v6.4S, v3.4S, v7.4S
    LD2             {v14.S, v15.S}[1], [X6] , X1

    ADD             v3.4S, v9.4S, v6.4S
    LD2             {v10.S, v11.S}[1], [X6] , X1

    SUB             v7.4S, v9.4S, v6.4S
    LD2             {v12.S, v13.S}[1], [X6] , X1

    SUB             v6.4S, v4.4S, v5.4S
    LD2             {v14.S, v15.S}[2], [X7] , X1

    ADD             v9.4S, v4.4S, v5.4S
    LD2             {v10.S, v11.S}[2], [X7] , X1

    ADD             v4.4S, v8.4S, v1.4S
    LD2             {v12.S, v13.S}[2], [X7] , X1

    SUB             v5.4S, v8.4S, v1.4S
    LD2             {v14.S, v15.S}[3], [X11] , X1

    ADD             v8.4S, v0.4S, v2.4S
    LD2             {v10.S, v11.S}[3], [X11] , X1

    SUB             v0.4S, v0.4S, v2.4S
    LD2             {v12.S, v13.S}[3], [X11] , X1












    LD2             {v1.S, v2.S}[0], [X5], X1

    ADD             v17.4S, v14.4S, v12.4S

    LD2             {v1.S, v2.S}[1], [X6] , X1

    SUB             v16.4S, v14.4S, v12.4S

    LD2             {v1.S, v2.S}[2], [X7] , X1

    ADD             v14.4S, v15.4S, v13.4S

    LD2             {v1.S, v2.S}[3], [X11] , X1

    SUB             v12.4S, v15.4S, v13.4S

    ADD             v15.4S, v10.4S, v1.4S
    SUB             v13.4S, v10.4S, v1.4S
    ADD             v10.4S, v11.4S, v2.4S
    SUB             v1.4S, v11.4S, v2.4S

    ADD             v11.4S, v17.4S, v15.4S
    SUB             v2.4S, v17.4S, v15.4S
    ADD             v17.4S, v14.4S, v10.4S
    SUB             v15.4S, v14.4S, v10.4S

    ADD             v14.4S, v16.4S, v12.4S
    SUB             v10.4S, v16.4S, v12.4S
    ADD             v16.4S, v13.4S, v1.4S
    SUB             v12.4S, v13.4S, v1.4S

    ADD             v1.4S , v14.4S, v12.4S
    SUB             v13.4S, v14.4S, v12.4S
    SUB             v12.4S, v16.4S, v10.4S


    UZP1            v22.8H, v1.8H, v1.8H
    UZP2            v23.8H, v1.8H, v1.8H
    ADD             v14.4S, v16.4S, v10.4S

    UZP1            v26.8H, v13.8H, v13.8H
    UZP2            v27.8H, v13.8H, v13.8H
    ADD             v16.4S, v4.4S, v11.4S

    UZP1            v24.8H, v12.8H, v12.8H
    UZP2            v25.8H, v12.8H, v12.8H
    SUB             v10.4S, v4.4S, v11.4S

    UZP1            v28.8H, v14.8H, v14.8H
    UZP2            v29.8H, v14.8H, v14.8H
    ADD             v4.4S, v8.4S, v17.4S

    MOv             W14, #0x5a82

    SUB             v11.4S, v8.4S, v17.4S

    ADD             v8.4S, v5.4S, v15.4S
    SUB             v17.4S, v5.4S, v15.4S
    SUB             v5.4S, v0.4S, v2.4S
    ADD             v15.4S, v0.4S, v2.4S





















    DUP             v31.4H, W14

    UMULL           v19.4S, v26.4H, v31.4H
    UMULL           v18.4S, v28.4H, v31.4H
    SSHR            v19.4S, v19.4S, #15
    SSHR            v18.4S, v18.4S, #15


    SQDMLAL         v19.4S, v27.4H, v31.4H
    SQDMLAL         v18.4S, v29.4H, v31.4H


    UMULL           v13.4S, v24.4H, v31.4H
    UMULL           v14.4S, v22.4H, v31.4H

    ADD             v20.4S, v3.4S, v19.4S
    SUB             v21.4S, v3.4S, v19.4S
    ADD             v30.4S, v6.4S, v18.4S
    SUB             v6.4S, v6.4S, v18.4S

    SSHR            v13.4S, v13.4S, #15
    SSHR            v14.4S, v14.4S, #15

    SQDMLAL         v13.4S, v25.4H, v31.4H
    SQDMLAL         v14.4S, v23.4H, v31.4H




    ADD             v3.4S, v7.4S, v13.4S
    SUB             v19.4S, v7.4S, v13.4S
    ADD             v1.4S, v9.4S, v14.4S
    SUB             v18.4S, v9.4S, v14.4S























    swp             v17.D[0], v8.D[0]
    swp             v17.D[1], v8.D[1]
    swp             v4.D[0], v16.D[0]
    swp             v4.D[1], v16.D[1]

    TRN1            v12.4S, v4.4S, v20.4S
    TRN2            v22.4S, v4.4S, v20.4S

    SHL             v12.4S, v12.4S, #3
    TRN1            v9.4S, v17.4S, v3.4S
    TRN2            v2.4S, v17.4S, v3.4S
    SHL             v22.4S, v22.4S, #3

    SHL             v9.4S, v9.4S, #3
    TRN1            v24.4S, v10.4S, v21.4S
    TRN2            v7.4S, v10.4S, v21.4S
    SHL             v2.4S, v2.4S, #3

    SHL             v24.4S, v24.4S, #3
    TRN1            v13.4S, v16.4S, v6.4S
    TRN2            v23.4S, v16.4S, v6.4S
    SHL             v7.4S, v7.4S, #3

    SHL             v13.4S, v13.4S, #3
    TRN1            v10.4S, v5.4S, v18.4S
    TRN2            v3.4S, v5.4S, v18.4S
    SHL             v23.4S, v23.4S, #3

    SHL             v10.4S, v10.4S, #3
    TRN1            v26.4S, v8.4S, v19.4S
    TRN2            v4.4S, v8.4S, v19.4S
    SHL             v3.4S, v3.4S, #3

    SHL             v26.4S, v26.4S, #3
    TRN1            v25.4S, v11.4S, v30.4S
    TRN2            v8.4S, v11.4S, v30.4S
    SHL             v4.4S, v4.4S, #3

    SHL             v25.4S, v25.4S, #3
    TRN1            v27.4S, v15.4S, v1.4S
    TRN2            v5.4S, v15.4S, v1.4S
    SHL             v8.4S, v8.4S, #3

    SHL             v27.4S, v27.4S, #3
    swp             v9.D[0], v12.D[1]
    SHL             v5.4S, v5.4S, #3
    swp             v2.D[0], v22.D[1]

    swp             v24.D[1], v26.D[0]
    swp             v7.D[1], v4.D[0]
    swp             v10.D[0], v13.D[1]
    swp             v3.D[0], v23.D[1]
    swp             v27.D[0], v25.D[1]
    swp             v5.D[0], v8.D[1]

    MOv             X15, #32
    ST2             {v12.4S, v13.4S}, [X3], X15
    ST2             {v24.4S, v25.4S}, [X3], X15
    ST2             {v22.4S, v23.4S}, [X3], X15
    ST2             {v7.4S, v8.4S}, [X3], X15
    ST2             {v9.4S, v10.4S}, [X3], X15
    ST2             {v26.4S, v27.4S}, [X3], X15
    ST2             {v2.4S, v3.4S}, [X3], X15
    ST2             {v4.4S, v5.4S}, [X3], X15


    SUBS            X9, X9, #1
    BNE             RADIX_8_FIRST_LOOP

    LSR             X1, X1, #1
    LSL             X15, X1, #3
    SUB             X3, X3, X15

    MOv             X5, #8
    MOv             X4, #32
    LSR             X15, X1, #5
    MOv             X6, X15
    B               RADIX_4_FIRST_ENDS
RADIX_8_FIRST_ENDS:

RADIX_4_FIRST_START:

    LSR             W9, W1, #4
    LSL             W1, W1, #1
RADIX_4_LOOP:

    MOv             X5 , X2
    MOv             X6 , X2
    MOv             X7 , X2
    MOv             X11 , X2















    LDRB            W12, [X4, #0]
    ADD             X5, X5, X12, LSL #3

    LD2             {v0.S, v1.S}[0], [X5] , X1
    ADD             X5, X5, X1
    LD2             {v8.S, v9.S}[0], [X5] , X1
    SUB             X5, X5, X1, LSL #1
    LD2             {v4.S, v5.S}[0], [X5] , X1
    ADD             X5, X5, X1
    LD2             {v12.S, v13.S}[0], [X5] , X1

    LDRB            W12, [X4, #1]
    ADD             X6, X6, X12, LSL #3
    LD2             {v0.S, v1.S}[1], [X6] , X1
    ADD             X6, X6, X1
    LD2             {v8.S, v9.S}[1], [X6] , X1
    SUB             X6, X6, X1, LSL #1
    LD2             {v4.S, v5.S}[1], [X6] , X1
    ADD             X6, X6, X1
    LD2             {v12.S, v13.S}[1], [X6] , X1

    LDRB            W12, [X4, #2]
    ADD             X7, X7, X12, LSL #3

    LD2             {v0.S, v1.S}[2], [X7] , X1
    ADD             X7, X7, X1
    LD2             {v8.S, v9.S}[2], [X7] , X1


    LDRB            W12, [X4, #3]
    ADD             X11, X11, X12 , LSL #3


    LD2             {v0.S, v1.S}[3], [X11] , X1
    ADD             X11, X11, X1
    LD2             {v8.S, v9.S}[3], [X11] , X1

    SUB             X7, X7, X1, LSL #1
    ADD             v16.4S, v0.4S, v8.4S
    LD2             {v4.S, v5.S}[2], [X7] , X1
    ADD             X7, X7, X1
    ADD             v18.4S, v1.4S, v9.4S
    LD2             {v12.S, v13.S}[2], [X7] , X1

    SUB             X11, X11, X1, LSL #1
    SUB             v20.4S, v0.4S, v8.4S
    LD2             {v4.S, v5.S}[3], [X11] , X1
    ADD             X11, X11, X1
    SUB             v22.4S, v1.4S, v9.4S
    LD2             {v12.S, v13.S}[3], [X11] , X1






    ADD             X4, X4, #4

    ADD             v24.4S, v4.4S, v12.4S
    ADD             v26.4S, v5.4S, v13.4S
    SUB             v28.4S, v4.4S, v12.4S
    SUB             v30.4S, v5.4S, v13.4S

    ADD             v17.4S, v16.4S, v24.4S
    ADD             v11.4S, v18.4S, v26.4S
    SUB             v19.4S, v16.4S, v24.4S
    SUB             v15.4S, v18.4S, v26.4S

    ADD             v8.4S, v20.4S, v30.4S
    SUB             v9.4S, v22.4S, v28.4S
    ADD             v13.4S, v22.4S, v28.4S
    SUB             v12.4S, v20.4S, v30.4S




    TRN1            v0.4S, v17.4S, v8.4S
    TRN2            v8.4S, v17.4S, v8.4S

    SHL             v0.4S, v0.4S, #2
    TRN1            v4.4S, v19.4S, v12.4S
    TRN2            v12.4S, v19.4S, v12.4S
    SHL             v8.4S, v8.4S, #2

    SHL             v4.4S, v4.4S, #2
    TRN1            v1.4S, v11.4S, v9.4S
    TRN2            v9.4S, v11.4S, v9.4S
    SHL             v12.4S, v12.4S, #2

    SHL             v1.4S, v1.4S, #2
    TRN1            v5.4S, v15.4S, v13.4S
    TRN2            v13.4S, v15.4S, v13.4S
    SHL             v9.4S, v9.4S, #2

    SHL             v5.4S, v5.4S, #2
    swp             v4.D[0], v0.D[1]
    SHL             v13.4S, v13.4S, #2

    swp             v12.D[0], v8.D[1]
    swp             v5.D[0], v1.D[1]
    swp             v13.D[0], v9.D[1]

    MOv             X15, #32
    ST2             {v0.4S, v1.4S}, [X3], X15
    ST2             {v8.4S, v9.4S}, [X3], X15
    ST2             {v4.4S, v5.4S}, [X3], X15
    ST2             {v12.4S, v13.4S}, [X3], X15


    SUBS            W9, W9, #1
    BNE             RADIX_4_LOOP

    LSR             X1, X1, #1
    SUB             X3, X3, X1, LSL #3
    MOv             X5, #4
    MOv             X4, #64
    LSR             X6, X1, #4


RADIX_4_FIRST_ENDS:

    MOv             x30, X3
    LSR             X5, X5, #2

    MOV             X14, #8528
    ADD             X0, X0, X14

OUTER_LOOP_R4:

    MOv             X14, x30

    MOv             X7, X5
    MOv             X2, #0
    MOv             X9, X0
    LSL             X12, X5, #5
MIDDLE_LOOP_R4:

    LD2             {v20.H, v21.H}[0], [X9], X2
    LD2             {v22.H, v23.H}[0], [X9], X2
    ADD             X11, X2, X4, LSL #2
    LD2             {v24.H, v25.H}[0], [X9]
    ADD             X10, X0, X11

    LD2             {v20.H, v21.H}[1], [X10], X11
    LD2             {v22.H, v23.H}[1], [X10], X11
    ADD             X2, X11, X4, LSL #2
    LD2             {v24.H, v25.H}[1], [X10]
    ADD             X9, X0, X2

    LD2             {v20.H, v21.H}[2], [X9], X2
    LD2             {v22.H, v23.H}[2], [X9], X2
    ADD             X11, X2, X4, LSL #2
    LD2             {v24.H, v25.H}[2], [X9]
    ADD             X10, X0, X11

    LD2             {v20.H, v21.H}[3], [X10], X11
    LD2             {v22.H, v23.H}[3], [X10], X11
    ADD             X2, X11, X4, LSL #2
    LD2             {v24.H, v25.H}[3], [X10]
    ADD             X9, X0, X2

    MOv             X10, X6
INNER_LOOP_R4:

    LD2             {v30.4S, v31.4S}, [X14], X12
    SSHR            v30.4S, v30.4S, #1
    LD4             {v16.4H, v17.4H, v18.4H, v19.4H}, [X14], X12
    SSHR            v31.4S, v31.4S, #1

    USHR            v16.4H, v16.4H, #1
    LD4             {v26.4H, v27.4H, v28.4H, v29.4H}, [X14], X12
    USHR            v18.4H, v18.4H, #1

    SMULL           v11.4S, v16.4H, v20.4H
    SMLSL           v11.4S, v18.4H, v21.4H

    LD4             {v0.4H, v1.4H, v2.4H, v3.4H}, [X14], X12
    SMULL           v12.4S, v16.4H, v21.4H
    SMLAL           v12.4S, v18.4H, v20.4H

    USHR            v26.4H, v26.4H, #1
    USHR            v28.4H, v28.4H, #1

    LSL             x29, X12, #2
    SUB             X14, X14, X12, LSL #2

    USHR            v0.4H, v0.4H, #1
    USHR            v2.4H, v2.4H, #1

    SMULL           v13.4S, v26.4H, v22.4H
    SMLSL           v13.4S, v28.4H, v23.4H

    SSHR            v11.4S, v11.4S, #15

    SMULL           v14.4S, v26.4H, v23.4H
    SMLAL           v14.4S, v28.4H, v22.4H

    SMULL           v15.4S, v0.4H, v24.4H
    SMLSL           v15.4S, v2.4H, v25.4H

    SMLAL           v11.4S, v17.4H, v20.4H
    SMLSL           v11.4S, v19.4H, v21.4H

    SSHR            v12.4S, v12.4S, #15
    SSHR            v13.4S, v13.4S, #15
    SSHR            v14.4S, v14.4S, #15
    SSHR            v15.4S, v15.4S, #15

    SMLAL           v12.4S, v17.4H, v21.4H
    SMLAL           v12.4S, v19.4H, v20.4H

    SMULL           v5.4S, v0.4H, v25.4H
    SMLAL           v5.4S, v2.4H, v24.4H

    SMLAL           v13.4S, v27.4H, v22.4H
    SMLSL           v13.4S, v29.4H, v23.4H

    SMLAL           v14.4S, v27.4H, v23.4H
    SMLAL           v14.4S, v29.4H, v22.4H

    SMLAL           v15.4S, v1.4H, v24.4H
    SMLSL           v15.4S, v3.4H, v25.4H

    SSHR            v5.4S, v5.4S, #15

    SMLAL           v5.4S, v1.4H, v25.4H
    SMLAL           v5.4S, v3.4H, v24.4H



    SUBS            x17, X7, X5
    BNE             BYPASS_IF

    ADD             X14, X14, X12

    LDR             W3, [X14]
    ADD             X14, X14, X12
    ASR             W3, W3, #1

    MOv             v11.S[0], W3

    LDR             W3, [X14]
    ADD             X14, X14, X12
    ASR             W3, W3, #1
    MOv             v13.S[0], W3

    LDR             W3, [X14]
    ASR             W3, W3, #1
    MOv             v15.S[0], W3

    SUB             X14, X14, X12, LSL #1
    ADD             X14, X14, #4

    LDR             W3, [X14]
    ADD             X14, X14, X12
    ASR             W3, W3, #1
    MOv             v12.S[0], W3

    LDR             W3, [X14]
    ADD             X14, X14, X12
    ASR             W3, W3, #1
    MOv             v14.S[0], W3

    LDR             W3, [X14]
    ADD             X14, X14, X12
    ASR             W3, W3, #1
    MOv             v5.S[0], W3

    SUB             X14, X14, #4

    SUB             X14, X14, x29








BYPASS_IF:

    ADD             v6.4S, v30.4S, v13.4S
    ADD             v7.4S, v31.4S, v14.4S
    SUB             v30.4S, v30.4S, v13.4S
    SUB             v31.4S, v31.4S, v14.4S
    ADD             v8.4S, v11.4S, v15.4S
    ADD             v9.4S, v12.4S, v5.4S

    SUB             v15.4S, v11.4S, v15.4S
    SUB             v14.4S, v12.4S, v5.4S


    ADD             v10.4S, v6.4S, v8.4S
    ADD             v11.4S, v7.4S, v9.4S
    ADD             v12.4S, v30.4S, v14.4S
    SUB             v13.4S, v31.4S, v15.4S

    SUB             v6.4S, v6.4S, v8.4S
    ST2             {v10.4S, v11.4S}, [X14], X12
    SUB             v7.4S, v7.4S, v9.4S

    SUB             v8.4S, v30.4S, v14.4S
    ST2             {v12.4S, v13.4S}, [X14], X12
    ADD             v9.4S, v31.4S, v15.4S

    ST2             {v6.4S, v7.4S}, [X14], X12
    ST2             {v8.4S, v9.4S}, [X14], X12
    SUBS            X10, X10, #1
    BNE             INNER_LOOP_R4

    SUB             X14, X14, X1, LSL #3
    ADD             X14, X14, #32

    SUBS            X7, X7, #1
    BNE             MIDDLE_LOOP_R4




    LSR             X4, X4, #2
    LSL             X5, X5, #2
    LSR             X6, X6, #2
    SUBS            X8, X8, #1
    BNE             OUTER_LOOP_R4
END_LOOPS:
    pop_v_regs
    RET



