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

.macro swp reg1, reg2
    MOV             x16, \reg1
    MOV             \reg1, \reg2
    MOV             \reg2, x16
.endm
.text
.p2align 2
.global ixheaacd_sbr_imdct_using_fft
ixheaacd_sbr_imdct_using_fft:
    push_v_regs


COND_6: cmp         x1, #0x10
    bne             COND_7
    MOV             X8, #1
    MOV             X4, X7
    B               RADIX_4_FIRST_START

COND_7: cmp         x1, #0x20

    mov             x8, #1
    mov             x4, x7


RADIX_8_FIRST_START:

    LSR             W9 , W1, #5
    LSL             W1, W1, #1

RADIX_8_FIRST_LOOP:

    MOV             X5 , X2
    MOV             X6 , X2
    MOV             X7 , X2
    MOV             X11 , X2






















    LDRB            W12, [X4]
    ADD             X5, X5, X12, LSL #3
    LD2             {V0.S, V1.S}[0], [X5], X1
    ADD             X5, X5, X1
    LD2             {V4.S, V5.S}[0], [X5], X1
    SUB             X5, X5, X1, LSL #1
    LD2             {V2.S, V3.S}[0], [X5], X1
    ADD             X5, X5, X1
    LD2             {V6.S, V7.S}[0], [X5], X1
    SUB             X5, X5, X1, LSL #2

    LDRB            W12, [X4, #1]
    ADD             X6, X6, X12, LSL #3
    LD2             {V0.S, V1.S}[1], [X6] , X1
    ADD             X6, X6, X1
    LD2             {V4.S, V5.S}[1], [X6] , X1
    SUB             X6, X6, X1, LSL #1
    LD2             {V2.S, V3.S}[1], [X6] , X1
    ADD             X6, X6, X1
    LD2             {V6.S, V7.S}[1], [X6], X1
    SUB             X6, X6, X1, LSL #2


    LDRB            W12, [X4, #2]
    ADD             X7, X7, X12, LSL #3
    LD2             {V0.S, V1.S}[2], [X7] , X1
    ADD             X7, X7, X1
    LD2             {V4.S, V5.S}[2], [X7] , X1
    SUB             X7, X7, X1, LSL #1

    LDRB            W12, [X4, #3]
    ADD             X11, X11, X12, LSL #3
    LD2             {V0.S, V1.S}[3], [X11] , X1
    ADD             X11, X11, X1
    LD2             {V4.S, V5.S}[3], [X11] , X1
    SUB             X11, X11, X1, LSL #1


    ADD             V8.4S, V0.4S, V4.4S
    LD2             {V2.S, V3.S}[2], [X7] , X1
    ADD             X7, X7, X1


    SUB             V9.4S, V0.4S, V4.4S
    LD2             {V6.S, V7.S}[2], [X7], X1
    SUB             X7, X7, X1, LSL #2


    ADD             V0.4S, V1.4S, V5.4S
    LD2             {V2.S, V3.S}[3], [X11] , X1
    ADD             X11, X11, X1

    SUB             V4.4S, V1.4S, V5.4S
    LD2             {V6.S, V7.S}[3], [X11], X1
    SUB             X11, X11, X1, LSL #2

    ADD             X4, X4, #4

    ADD             X5, X5, X1, LSR #1
    ADD             X6, X6, X1, LSR #1
    ADD             X7, X7, X1, LSR #1
    ADD             X11, X11, X1, LSR #1


    ADD             V1.4S, V2.4S, V6.4S
    LD2             {V14.S, V15.S}[0], [X5] , X1


    SUB             V5.4S, V2.4S, V6.4S
    LD2             {V10.S, V11.S}[0], [X5] , X1


    ADD             V2.4S, V3.4S, V7.4S
    LD2             {V12.S, V13.S}[0], [X5] , X1


    SUB             V6.4S, V3.4S, V7.4S
    LD2             {V14.S, V15.S}[1], [X6] , X1

    ADD             V3.4S, V9.4S, V6.4S
    LD2             {V10.S, V11.S}[1], [X6] , X1

    SUB             V7.4S, V9.4S, V6.4S
    LD2             {V12.S, V13.S}[1], [X6] , X1

    SUB             V6.4S, V4.4S, V5.4S
    LD2             {V14.S, V15.S}[2], [X7] , X1

    ADD             V9.4S, V4.4S, V5.4S
    LD2             {V10.S, V11.S}[2], [X7] , X1

    ADD             V4.4S, V8.4S, V1.4S
    LD2             {V12.S, V13.S}[2], [X7] , X1

    SUB             V5.4S, V8.4S, V1.4S
    LD2             {V14.S, V15.S}[3], [X11] , X1

    ADD             V8.4S, V0.4S, V2.4S
    LD2             {V10.S, V11.S}[3], [X11] , X1

    SUB             V0.4S, V0.4S, V2.4S
    LD2             {V12.S, V13.S}[3], [X11] , X1


    LD2             {V1.S, V2.S}[0], [X5], X1

    ADD             V17.4S, V14.4S, V12.4S

    LD2             {V1.S, V2.S}[1], [X6] , X1

    SUB             V16.4S, V14.4S, V12.4S

    LD2             {V1.S, V2.S}[2], [X7] , X1

    ADD             V14.4S, V15.4S, V13.4S

    LD2             {V1.S, V2.S}[3], [X11] , X1

    SUB             V12.4S, V15.4S, V13.4S

    ADD             V15.4S, V10.4S, V1.4S
    SUB             V13.4S, V10.4S, V1.4S
    ADD             V10.4S, V11.4S, V2.4S
    SUB             V1.4S, V11.4S, V2.4S

    ADD             V11.4S, V17.4S, V15.4S
    SUB             V2.4S, V17.4S, V15.4S
    ADD             V17.4S, V14.4S, V10.4S
    SUB             V15.4S, V14.4S, V10.4S

    ADD             V14.4S, V16.4S, V12.4S
    SUB             V10.4S, V16.4S, V12.4S
    ADD             V16.4S, V13.4S, V1.4S
    SUB             V12.4S, V13.4S, V1.4S

    ADD             V1.4S , V14.4S, V12.4S
    SUB             V13.4S, V14.4S, V12.4S
    SUB             V12.4S, V16.4S, V10.4S

    UZP1            V22.8H, V1.8H, V1.8H
    UZP2            V23.8H, V1.8H, V1.8H
    ADD             V14.4S, V16.4S, V10.4S

    UZP1            V26.8H, V13.8H, V13.8H
    UZP2            V27.8H, V13.8H, V13.8H
    ADD             V16.4S, V4.4S, V11.4S

    UZP1            V24.8H, V12.8H, V12.8H
    UZP2            V25.8H, V12.8H, V12.8H
    SUB             V10.4S, V4.4S, V11.4S

    UZP1            V28.8H, V14.8H, V14.8H
    UZP2            V29.8H, V14.8H, V14.8H
    ADD             V4.4S, V8.4S, V17.4S

    MOV             W14, #0x5a82

    SUB             V11.4S, V8.4S, V17.4S

    ADD             V8.4S, V5.4S, V15.4S
    SUB             V17.4S, V5.4S, V15.4S
    SUB             V5.4S, V0.4S, V2.4S
    ADD             V15.4S, V0.4S, V2.4S















    DUP             V31.4H, W14

    UMULL           V19.4S, V26.4H, V31.4H
    UMULL           V18.4S, V28.4H, V31.4H
    SSHR            V19.4S, V19.4S, #15
    SSHR            V18.4S, V18.4S, #15

    SQDMLAL         V19.4S, V27.4H, V31.4H
    SQDMLAL         V18.4S, V29.4H, V31.4H

    UMULL           V13.4S, V24.4H, V31.4H
    UMULL           V14.4S, V22.4H, V31.4H

    ADD             V20.4S, V3.4S, V19.4S
    SUB             V21.4S, V3.4S, V19.4S
    ADD             V30.4S, V6.4S, V18.4S
    SUB             V6.4S, V6.4S, V18.4S

    SSHR            V13.4S, V13.4S, #15
    SSHR            V14.4S, V14.4S, #15

    SQDMLAL         V13.4S, V25.4H, V31.4H
    SQDMLAL         V14.4S, V23.4H, V31.4H

    ADD             V3.4S, V7.4S, V13.4S
    SUB             V19.4S, V7.4S, V13.4S
    ADD             V1.4S, V9.4S, V14.4S
    SUB             V18.4S, V9.4S, V14.4S

























    swp             V17.D[0], V8.D[0]
    swp             V17.D[1], V8.D[1]
    swp             V4.D[0], V16.D[0]
    swp             V4.D[1], V16.D[1]

    TRN1            V12.4S, V4.4S, V20.4S
    TRN2            V22.4S, V4.4S, V20.4S

    SHL             V12.4S, V12.4S, #1
    TRN1            V9.4S, V17.4S, V3.4S
    TRN2            V2.4S, V17.4S, V3.4S
    SHL             V22.4S, V22.4S, #1

    SHL             V9.4S, V9.4S, #1
    TRN1            V24.4S, V10.4S, V21.4S
    TRN2            V7.4S, V10.4S, V21.4S
    SHL             V2.4S, V2.4S, #1

    SHL             V24.4S, V24.4S, #1
    TRN1            V13.4S, V16.4S, V6.4S
    TRN2            V23.4S, V16.4S, V6.4S
    SHL             V7.4S, V7.4S, #1

    SHL             V13.4S, V13.4S, #1
    TRN1            V10.4S, V5.4S, V18.4S
    TRN2            V3.4S, V5.4S, V18.4S
    SHL             V23.4S, V23.4S, #1

    SHL             V10.4S, V10.4S, #1
    TRN1            V26.4S, V8.4S, V19.4S
    TRN2            V4.4S, V8.4S, V19.4S
    SHL             V3.4S, V3.4S, #1

    SHL             V26.4S, V26.4S, #1
    TRN1            V25.4S, V11.4S, V30.4S
    TRN2            V8.4S, V11.4S, V30.4S
    SHL             V4.4S, V4.4S, #1

    SHL             V25.4S, V25.4S, #1
    TRN1            V27.4S, V15.4S, V1.4S
    TRN2            V5.4S, V15.4S, V1.4S
    SHL             V8.4S, V8.4S, #1

    SHL             V27.4S, V27.4S, #1
    swp             V9.D[0], V12.D[1]
    SHL             V5.4S, V5.4S, #1
    swp             V2.D[0], V22.D[1]

    swp             V24.D[1], V26.D[0]
    swp             V7.D[1], V4.D[0]
    swp             V10.D[0], V13.D[1]
    swp             V3.D[0], V23.D[1]
    swp             V27.D[0], V25.D[1]
    swp             V5.D[0], V8.D[1]


    MOV             X15, #32
    ST2             {V12.4S, V13.4S}, [X3], X15
    ST2             {V24.4S, V25.4S}, [X3], X15
    ST2             {V22.4S, V23.4S}, [X3], X15
    ST2             {V7.4S, V8.4S}, [X3], X15
    ST2             {V9.4S, V10.4S}, [X3], X15
    ST2             {V26.4S, V27.4S}, [X3], X15
    ST2             {V2.4S, V3.4S}, [X3], X15
    ST2             {V4.4S, V5.4S}, [X3], X15


    SUBS            X9, X9, #1
    BNE             RADIX_8_FIRST_LOOP

    LSR             X1, X1, #1
    LSL             X15, X1, #3
    SUB             X3, X3, X15

    MOV             X5, #8
    MOV             X4, #32
    LSR             X15, X1, #5
    MOV             X6, X15
    B               RADIX_4_FIRST_ENDS

RADIX_8_FIRST_ENDS:



RADIX_4_FIRST_START:


    LSR             W9, W1, #4
    LSL             W1, W1, #1

RADIX_4_LOOP:

    MOV             X5 , X2
    MOV             X6 , X2
    MOV             X7 , X2
    MOV             X11 , X2















    LDRB            W12, [X4, #0]
    ADD             X5, X5, X12, LSL #3

    LD2             {V0.S, V1.S}[0], [X5] , X1
    ADD             X5, X5, X1
    LD2             {V8.S, V9.S}[0], [X5] , X1
    SUB             X5, X5, X1, LSL #1
    LD2             {V4.S, V5.S}[0], [X5] , X1
    ADD             X5, X5, X1
    LD2             {V12.S, V13.S}[0], [X5] , X1

    LDRB            W12, [X4, #1]
    ADD             X6, X6, X12, LSL #3
    LD2             {V0.S, V1.S}[1], [X6] , X1
    ADD             X6, X6, X1
    LD2             {V8.S, V9.S}[1], [X6] , X1
    SUB             X6, X6, X1, LSL #1
    LD2             {V4.S, V5.S}[1], [X6] , X1
    ADD             X6, X6, X1
    LD2             {V12.S, V13.S}[1], [X6] , X1

    LDRB            W12, [X4, #2]
    ADD             X7, X7, X12, LSL #3

    LD2             {V0.S, V1.S}[2], [X7] , X1
    ADD             X7, X7, X1
    LD2             {V8.S, V9.S}[2], [X7] , X1


    LDRB            W12, [X4, #3]
    ADD             X11, X11, X12 , LSL #3


    LD2             {V0.S, V1.S}[3], [X11] , X1
    ADD             X11, X11, X1
    LD2             {V8.S, V9.S}[3], [X11] , X1

    SUB             X7, X7, X1, LSL #1
    ADD             V16.4S, V0.4S, V8.4S
    LD2             {V4.S, V5.S}[2], [X7] , X1
    ADD             X7, X7, X1
    ADD             V18.4S, V1.4S, V9.4S
    LD2             {V12.S, V13.S}[2], [X7] , X1

    SUB             X11, X11, X1, LSL #1
    SUB             V20.4S, V0.4S, V8.4S
    LD2             {V4.S, V5.S}[3], [X11] , X1
    ADD             X11, X11, X1
    SUB             V22.4S, V1.4S, V9.4S
    LD2             {V12.S, V13.S}[3], [X11] , X1

    ADD             X4, X4, #4

    ADD             V24.4S, V4.4S, V12.4S
    ADD             V26.4S, V5.4S, V13.4S
    SUB             V28.4S, V4.4S, V12.4S
    SUB             V30.4S, V5.4S, V13.4S

    ADD             V17.4S, V16.4S, V24.4S
    ADD             V11.4S, V18.4S, V26.4S
    SUB             V19.4S, V16.4S, V24.4S
    SUB             V15.4S, V18.4S, V26.4S

    ADD             V8.4S, V20.4S, V30.4S
    SUB             V9.4S, V22.4S, V28.4S
    ADD             V13.4S, V22.4S, V28.4S
    SUB             V12.4S, V20.4S, V30.4S




    TRN1            V0.4S, V17.4S, V8.4S
    TRN2            V8.4S, V17.4S, V8.4S

    SHL             V0.4S, V0.4S, #1
    TRN1            V4.4S, V19.4S, V12.4S
    TRN2            V12.4S, V19.4S, V12.4S
    SHL             V8.4S, V8.4S, #1

    SHL             V4.4S, V4.4S, #1
    TRN1            V1.4S, V11.4S, V9.4S
    TRN2            V9.4S, V11.4S, V9.4S
    SHL             V12.4S, V12.4S, #1

    SHL             V1.4S, V1.4S, #1
    TRN1            V5.4S, V15.4S, V13.4S
    TRN2            V13.4S, V15.4S, V13.4S
    SHL             V9.4S, V9.4S, #1

    SHL             V5.4S, V5.4S, #1
    swp             V4.D[0], V0.D[1]
    SHL             V13.4S, V13.4S, #1

    swp             V12.D[0], V8.D[1]


    swp             V5.D[0], V1.D[1]
    swp             V13.D[0], V9.D[1]

    MOV             X15, #32
    ST2             {V0.4S, V1.4S}, [X3], X15
    ST2             {V8.4S, V9.4S}, [X3], X15
    ST2             {V4.4S, V5.4S}, [X3], X15
    ST2             {V12.4S, V13.4S}, [X3], X15


    SUBS            W9, W9, #1
    BNE             RADIX_4_LOOP

    LSR             X1, X1, #1
    SUB             X3, X3, X1, LSL #3
    MOV             X5, #4
    MOV             X4, #64
    LSR             X6, X1, #4


RADIX_4_FIRST_ENDS:






















    MOV             x30, X3
    LSR             X5, X5, #2

OUTER_LOOP_R4:


    MOV             X14, x30

    MOV             X7, X5
    MOV             X2, #0
    MOV             X9, X0
    LSL             X12, X5, #5
MIDDLE_LOOP_R4:


    LD2             {V20.H, V21.H}[0], [X9], X2
    LD2             {V22.H, V23.H}[0], [X9], X2
    ADD             X11, X2, X4, LSL #2
    LD2             {V24.H, V25.H}[0], [X9]
    ADD             X10, X0, X11

    LD2             {V20.H, V21.H}[1], [X10], X11
    LD2             {V22.H, V23.H}[1], [X10], X11
    ADD             X2, X11, X4, LSL #2
    LD2             {V24.H, V25.H}[1], [X10]
    ADD             X9, X0, X2

    LD2             {V20.H, V21.H}[2], [X9], X2
    LD2             {V22.H, V23.H}[2], [X9], X2
    ADD             X11, X2, X4, LSL #2
    LD2             {V24.H, V25.H}[2], [X9]
    ADD             X10, X0, X11

    LD2             {V20.H, V21.H}[3], [X10], X11
    LD2             {V22.H, V23.H}[3], [X10], X11
    ADD             X2, X11, X4, LSL #2
    LD2             {V24.H, V25.H}[3], [X10]
    ADD             X9, X0, X2

    MOV             X10, X6
INNER_LOOP_R4:

    LD2             {V30.4S, V31.4S}, [X14], X12
    SSHR            V30.4S, V30.4S, #1
    LD4             {V16.4H, V17.4H, V18.4H, V19.4H}, [X14], X12
    SSHR            V31.4S, V31.4S, #1

    USHR            V16.4H, V16.4H, #1
    LD4             {V26.4H, V27.4H, V28.4H, V29.4H}, [X14], X12
    USHR            V18.4H, V18.4H, #1

    SMULL           V11.4S, V16.4H, V20.4H
    SMLSL           V11.4S, V18.4H, V21.4H
    LD4             {V0.4H, V1.4H, V2.4H, V3.4H}, [X14], X12
    SMULL           V12.4S, V16.4H, V21.4H
    SMLAL           V12.4S, V18.4H, V20.4H

    USHR            V26.4H, V26.4H, #1
    USHR            V28.4H, V28.4H, #1

    LSL             x29, X12, #2
    SUB             X14, X14, X12, LSL #2

    USHR            V0.4H, V0.4H, #1
    USHR            V2.4H, V2.4H, #1

    SMULL           V13.4S, V26.4H, V22.4H
    SMLSL           V13.4S, V28.4H, V23.4H

    SSHR            V11.4S, V11.4S, #15

    SMULL           V14.4S, V26.4H, V23.4H
    SMLAL           V14.4S, V28.4H, V22.4H

    SMULL           V15.4S, V0.4H, V24.4H
    SMLSL           V15.4S, V2.4H, V25.4H

    SMLAL           V11.4S, V17.4H, V20.4H
    SMLSL           V11.4S, V19.4H, V21.4H

    SSHR            V12.4S, V12.4S, #15
    SSHR            V13.4S, V13.4S, #15
    SSHR            V14.4S, V14.4S, #15
    SSHR            V15.4S, V15.4S, #15

    SMLAL           V12.4S, V17.4H, V21.4H
    SMLAL           V12.4S, V19.4H, V20.4H

    SMULL           V5.4S, V0.4H, V25.4H
    SMLAL           V5.4S, V2.4H, V24.4H

    SMLAL           V13.4S, V27.4H, V22.4H
    SMLSL           V13.4S, V29.4H, V23.4H

    SMLAL           V14.4S, V27.4H, V23.4H
    SMLAL           V14.4S, V29.4H, V22.4H

    SMLAL           V15.4S, V1.4H, V24.4H
    SMLSL           V15.4S, V3.4H, V25.4H

    SSHR            V5.4S, V5.4S, #15

    SMLAL           V5.4S, V1.4H, V25.4H
    SMLAL           V5.4S, V3.4H, V24.4H



    SUBS            x17, X7, X5
    BNE             BYPASS_IF

    ADD             X14, X14, X12

    LDR             W3, [X14]
    ADD             X14, X14, X12
    ASR             W3, W3, #1
    MOV             V11.S[0], W3

    LDR             W3, [X14]
    ADD             X14, X14, X12
    ASR             W3, W3, #1
    MOV             V13.S[0], W3

    LDR             W3, [X14]
    ASR             W3, W3, #1
    MOV             V15.S[0], W3

    SUB             X14, X14, X12, LSL #1
    ADD             X14, X14, #4

    LDR             W3, [X14]
    ADD             X14, X14, X12
    ASR             W3, W3, #1
    MOV             V12.S[0], W3

    LDR             W3, [X14]
    ADD             X14, X14, X12
    ASR             W3, W3, #1
    MOV             V14.S[0], W3

    LDR             W3, [X14]
    ADD             X14, X14, X12
    ASR             W3, W3, #1
    MOV             V5.S[0], W3

    SUB             X14, X14, #4

    SUB             X14, X14, x29

BYPASS_IF:

    ADD             V6.4S, V30.4S, V13.4S
    ADD             V7.4S, V31.4S, V14.4S
    SUB             V30.4S, V30.4S, V13.4S
    SUB             V31.4S, V31.4S, V14.4S
    ADD             V8.4S, V11.4S, V15.4S
    ADD             V9.4S, V12.4S, V5.4S

    SUB             V15.4S, V11.4S, V15.4S
    SUB             V14.4S, V12.4S, V5.4S


    ADD             V10.4S, V6.4S, V8.4S
    ADD             V11.4S, V7.4S, V9.4S
    ADD             V12.4S, V30.4S, V14.4S
    SUB             V13.4S, V31.4S, V15.4S

    SUB             V6.4S, V6.4S, V8.4S
    ST2             {V10.4S, V11.4S}, [X14], X12
    SUB             V7.4S, V7.4S, V9.4S

    SUB             V8.4S, V30.4S, V14.4S
    ST2             {V12.4S, V13.4S}, [X14], X12
    ADD             V9.4S, V31.4S, V15.4S

    ST2             {V6.4S, V7.4S}, [X14], X12
    ST2             {V8.4S, V9.4S}, [X14], X12
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
