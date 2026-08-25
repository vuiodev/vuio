@/******************************************************************************
@ *
@ * Copyright (C) 2018 The Android Open Source Project
@ *
@ * Licensed under the Apache License, Version 2.0 (the "License");
@ * you may not use this file except in compliance with the License.
@ * You may obtain a copy of the License at:
@ *
@ * http://www.apache.org/licenses/LICENSE-2.0
@ *
@ * Unless required by applicable law or agreed to in writing, software
@ * distributed under the License is distributed on an "AS IS" BASIS,
@ * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
@ * See the License for the specific language governing permissions and
@ * limitations under the License.
@ *
@ *****************************************************************************
@ * Originally developed and contributed by Ittiam Systems Pvt. Ltd, Bangalore
@*/


.text
.p2align 2
    .global ixheaacd_sbr_qmfanal32_winadds
    .type ixheaacd_sbr_qmfanal32_winadds, %function

ixheaacd_sbr_qmfanal32_winadds:

    STMFD           sp!, {R4-R12, R14}
    VPUSH           {D8 - D15}
    LDR             R5, [SP, #108]
    LDR             R6, [SP, #112]
    LDR             R7, [SP, #116]

    MOV             R9, R7, LSL #1

    ADD             r5, r5, #64
    MOV             r10, #3

LOOP:
    LDRSH           r4  , [R6], r9
    LDRSH           r8  , [R6], r9
    LDRSH           r11  , [R6], r9
    LDRSH           r12 , [R6], r9


    STRH            r4  , [r5 , #-2]!
    STRH            r8  , [r5 , #-2]!
    STRH            r11  , [r5 , #-2]!
    STRH            r12 , [r5 , #-2]!

    LDRSH           r4  , [R6], r9
    LDRSH           r8  , [R6], r9
    LDRSH           r11  , [R6], r9
    LDRSH           r12 , [R6], r9


    STRH            r4  , [r5 , #-2]!
    STRH            r8  , [r5 , #-2]!
    STRH            r11  , [r5 , #-2]!
    STRH            r12 , [r5 , #-2]!


    SUBS            r10, r10, #1

    BPL             LOOP

    LDR             R4, [SP, #104]

    MOV             R5, #8
    VLD1.16         D0, [R0]!
    MOV             R6, #64

    MOV             R6, R6, LSL #1
    VLD2.16         {D1, D2}, [R2]!
    MOV             R7, #244

    MOV             R9, R0
    ADD             R0, R0, #120

    MOV             R11, R4
    VLD1.16         D2, [R0], R6
    ADD             R11, R11, #128




    MOV             R10, R2
    ADD             R2, R2, #240

    VMULL.S16       Q15, D0, D1
    VLD2.16         {D3, D4}, [R2]!
    ADD             R2, R2, #240


    VLD1.16         D4, [R0], R6
    VMLAL.S16       Q15, D2, D3

    VLD2.16         {D5, D6}, [R2]!


    ADD             R2, R2, #240
    VLD1.16         D6, [R0], R6
    VMLAL.S16       Q15, D4, D5

    VLD2.16         {D7, D8}, [R2]!


    ADD             R2, R2, #240
    VLD1.16         D8, [R0], R6
    VMLAL.S16       Q15, D6, D7

    MOV             R0, R9
    VLD2.16         {D9, D10}, [R2]!


    ADD             R2, R2, #240
    VLD1.16         D10, [R1]!
    VMLAL.S16       Q15, D8, D9



    MOV             R9, R1
    VLD2.16         {D11, D12}, [R3]!
    ADD             R1, R1, #120


    MOV             R2, R10
    VLD1.16         D12, [R1], R6
    MOV             R10, R3

    ADD             R3, R3, #240
    VLD2.16         {D13, D14}, [R3]!
    ADD             R3, R3, #240


    VLD2.16         {D15, D16}, [R3]!

    VLD1.16         D14, [R1], R6
    ADD             R3, R3, #240



    VLD1.16         D16, [R1], R6
    SUB             R5, R5, #1

    VLD2.16         {D17, D18}, [R3]!


    ADD             R3, R3, #240
    VLD1.16         D18, [R1], R6

    MOV             R1, R9
    VLD2.16         {D19, D20}, [R3]!

    ADD             R3, R3, #240

    MOV             R3, R10


LOOP_1:


    VLD1.16         D0, [R0]!

    MOV             R9, R0
    VLD2.16         {D1, D2}, [R2]!
    ADD             R0, R0, #120

    MOV             R10, R2
    VST1.32         {Q15}, [R4]!
    ADD             R2, R2, #240


    VMULL.S16       Q15, D10, D11
    VLD1.16         D2, [R0], R6
    VMLAL.S16       Q15, D12, D13

    VMLAL.S16       Q15, D14, D15
    VLD2.16         {D3, D4}, [R2]!
    VMLAL.S16       Q15, D16, D17

    VMLAL.S16       Q15, D18, D19
    VLD1.16         D4, [R0], R6
    ADD             R2, R2, #240

    VST1.32         {Q15}, [R11]!


    VMULL.S16       Q15, D0, D1
    VLD2.16         {D5, D6}, [R2]!
    VMLAL.S16       Q15, D2, D3



    ADD             R2, R2, #240
    VLD1.16         D6, [R0], R6
    VMLAL.S16       Q15, D4, D5

    VLD2.16         {D7, D8}, [R2]!


    ADD             R2, R2, #240
    VLD1.16         D8, [R0], R6
    VMLAL.S16       Q15, D6, D7

    MOV             R0, R9
    VLD2.16         {D9, D10}, [R2]!



    ADD             R2, R2, #240
    VLD1.16         D10, [R1]!
    MOV             R2, R10

    MOV             R9, R1
    VLD2.16         {D11, D12}, [R3]!
    ADD             R1, R1, #120


    VMLAL.S16       Q15, D8, D9
    VLD1.16         D12, [R1], R6
    MOV             R10, R3


    ADD             R3, R3, #240
    VLD2.16         {D13, D14}, [R3]!
    ADD             R3, R3, #240



    VLD1.16         D14, [R1], R6
    VLD2.16         {D15, D16}, [R3]!
    ADD             R3, R3, #240


    VLD1.16         D16, [R1], R6
    VLD2.16         {D17, D18}, [R3]!
    ADD             R3, R3, #240


    VLD1.16         D18, [R1], R6
    SUBS            R5, R5, #1

    MOV             R1, R9
    VLD2.16         {D19, D20}, [R3]!

    ADD             R3, R3, #240

    MOV             R3, R10

    BGT             LOOP_1

    VST1.32         {Q15}, [R4]!
    VMULL.S16       Q15, D10, D11
    VMLAL.S16       Q15, D12, D13

    VMLAL.S16       Q15, D14, D15
    VMLAL.S16       Q15, D16, D17
    VMLAL.S16       Q15, D18, D19

    VST1.32         {Q15}, [R11]!

    VPOP            {D8 - D15}
    LDMFD           sp!, {R4-R12, R15}

