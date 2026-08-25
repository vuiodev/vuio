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
    .global ixheaacd_over_lap_add2_armv7

ixheaacd_over_lap_add2_armv7:

    STMFD           sp!, {R4-R12, R14}
    VPUSH           {d8 - d15}

    LDR             R4, [SP, #104]
    LDR             R5, [SP, #108]
    LDR             R6, [SP, #112]
    RSB             R4, R4, #15
    CMP             R4, #31
    MOVGT           R4, #31
    SUB             R9, R4, #1
    MOV             R8, #1
    MOV             R8, R8, LSL R9
    RSB             R4, R4, #0
    VDUP.32         Q11, R4
    VDUP.32         Q10, R8
    MOV             R8, R5
    SUB             R12, R5, #1
    MOV             R9, R5, LSL #2
    MOV             R12, R12, LSL #2
    ADD             R10, R0, R9
    ADD             R7, R1, R12
    VLD2.16         {D0, D1}, [R10]!
    MOV             R11, R6, LSL #2
    SUB             R7, R7, #12
    ADD             R4, R4, #1
    MOV             R12, #-16
    VLD2.16         {D6, D7}, [R7], R12
    MOV             R4, #0x8000
    VREV64.16       D4, D6
    VREV64.16       D5, D7
    MOV             R4, R3

    MOV             R9, R2
    VLD2.16         {D2, D3}, [R3]!

    VMULL.U16       Q13, D0, D2
    VMLSL.U16       Q13, D4, D3
    VLD2.16         {D8, D9}, [R10]!
    VSHR.S32        Q13, Q13, #16
    VLD2.16         {D10, D11}, [R3]!
    VMLAL.S16       Q13, D1, D2
    VMLSL.S16       Q13, D5, D3
    VLD2.16         {D14, D15}, [R7], R12
    VREV64.16       Q6, Q7
    VQADD.S32       Q12, Q13, Q10
    VQSHL.S32       Q12, Q12, Q11
    SUB             R8, R8, #8


LOOP_1:

    VLD2.16         {D0, D1}, [R10]!
    VMULL.U16       Q9, D8, D10
    VLD2.16         {D2, D3}, [R3]!
    VMLSL.U16       Q9, D12, D11
    VLD2.16         {D6, D7}, [R7], R12
    VMULL.U16       Q13, D0, D2
    VREV64.16       D4, D6
    VMLSL.U16       Q13, D4, D3
    VREV64.16       D5, D7
    VSHR.S32        Q9, Q9, #16
    VST1.32         {D24[0]}, [R2], R11
    VMLAL.S16       Q9, D9, D10
    VST1.32         {D24[1]}, [R2], R11
    VSHR.S32        Q13, Q13, #16
    VST1.32         {D25[0]}, [R2], R11
    VMLAL.S16       Q13, D1, D2

    VST1.32         {D25[1]}, [R2], R11
    VMLSL.S16       Q9, D13, D11
    VMLSL.S16       Q13, D5, D3

    VLD2.16         {D8, D9}, [R10]!
    VLD2.16         {D10, D11}, [R3]!


    VLD2.16         {D14, D15}, [R7], R12
    VQADD.S32       Q8, Q9, Q10
    VREV64.16       Q6, Q7
    VQADD.S32       Q12, Q13, Q10
    VQSHL.S32       Q8, Q8, Q11
    VST1.32         D16[0], [R2], R11
    VQSHL.S32       Q12, Q12, Q11


    SUBS            R8, R8, #8

    VST1.32         D16[1], [R2], R11
    VST1.32         D17[0], [R2], R11
    VST1.32         D17[1], [R2], R11


    BGT             LOOP_1


    VST1.32         D24[0], [R2], R11
    VMULL.U16       Q9, D8, D10
    VMLSL.U16       Q9, D12, D11
    VST1.32         D24[1], [R2], R11
    VST1.32         D25[0], [R2], R11
    VSHR.S32        Q9, Q9, #16
    VST1.32         D25[1], [R2], R11
    VMLAL.S16       Q9, D9, D10
    VMLSL.S16       Q9, D13, D11
    MOV             R12, #12
    SMULBB          R7, R5, R6
    MOV             R10, R5, LSL #1
    VQADD.S32       Q8, Q9, Q10
    VQSHL.S32       Q8, Q8, Q11

    VST1.32         D16[0], [R2], R11
    MOV             R7, R7, LSL #2

    VST1.32         D16[1], [R2], R11
    ADD             R7, R7, R9

    VST1.32         D17[0], [R2], R11
    VST1.32         D17[1], [R2], R11

    SUB             R11, R10, #1
    MOV             R10, R11, LSL #2
    ADD             R10, R0, R10
    MOV             R11, R11, LSL #1
    SUB             R10, R10, R12
    MOV             R8, R6, LSL #2
    MOV             R12, #-16
    ADD             R11, R11, R4

    VLD1.32         {D6, D7}, [R10], R12
    SUB             R11, R11, #14


    VREV64.32       D0, D6
    VREV64.32       D1, D7
    VQNEG.S32       D0, D0
    VQNEG.S32       D1, D1
    VUZP.16         D1, D0
    VLD2.16         {D2, D3}, [R11], R12
    VREV64.16       D2, D2
    VREV64.16       D3, D3

    VLD2.16         {D4, D5}, [R1]!

    VMULL.U16       Q13, D1, D3
    VMLSL.U16       Q13, D4, D2
    VSHR.S32        Q13, Q13, #16
    VMLAL.S16       Q13, D0, D3
    VMLSL.S16       Q13, D5, D2
    @VQSHL.S32 Q12,Q13,Q11
    @VQADD.S32 Q12,Q12,Q10
    @VSHR.S32 Q12,Q12,#16
    VQADD.S32       Q12, Q13, Q10
    VQSHL.S32       Q12, Q12, Q11
    VUZP.16         D24, D25


    VLD1.32         {D14, D15}, [R10], R12
    VMULL.U16       Q13, D1, D3
    VMLSL.U16       Q13, D4, D2
    VREV64.32       Q4, Q7
    VQNEG.S32       Q4, Q4
    VLD2.16         {D10, D11}, [R11], R12
    VSHR.S32        Q13, Q13, #16
    VLD2.16         {D12, D13}, [R1]!
    VMLAL.S16       Q13, D0, D3
    VMLSL.S16       Q13, D5, D2
    VUZP.16         D9, D8
    VREV64.16       Q5, Q5
    VQADD.S32       Q12, Q13, Q10
    SUB             R5, R5, #8
    VQSHL.S32       Q12, Q12, Q11





LOOP_2:


    VLD1.32         {D6, D7}, [R10], R12
    VMULL.U16       Q9, D9, D11
    VREV64.32       Q0, Q3
    VQNEG.S32       Q0, Q0
    VUZP.16         D1, D0
    VLD2.16         {D2, D3}, [R11], R12
    VREV64.16       Q1, Q1

    VLD2.16         {D4, D5}, [R1]!
    VMLSL.U16       Q9, D12, D10
    VST1.32         D24[0], [R7], R8
    VMULL.U16       Q13, D1, D3
    VST1.32         D24[1], [R7], R8
    VSHR.S32        Q9, Q9, #16
    VST1.32         D25[0], [R7], R8
    VMLSL.U16       Q13, D4, D2
    VST1.32         D25[1], [R7], R8
    VMLAL.S16       Q9, D8, D11
    VLD1.32         {D14, D15}, [R10], R12
    VSHR.S32        Q13, Q13, #16
    VMLSL.S16       Q9, D13, D10
    VLD2.16         {D10, D11}, [R11], R12
    VMLAL.S16       Q13, D0, D3
    VMLSL.S16       Q13, D5, D2
    VREV64.32       Q4, Q7
    VLD2.16         {D12, D13}, [R1]!
    VQNEG.S32       Q4, Q4
    VREV64.16       Q5, Q5
    VQADD.S32       Q8, Q9, Q10
    VUZP.16         D9, D8
    VQADD.S32       Q12, Q13, Q10
    VQSHL.S32       Q8, Q8, Q11
    SUBS            R5, R5, #8
    VST1.32         D16[0], [R7], R8
    VQSHL.S32       Q12, Q12, Q11
    VST1.32         D16[1], [R7], R8

    VST1.32         D17[0], [R7], R8
    VST1.32         D17[1], [R7], R8

    BGT             LOOP_2

    VST1.32         D24[0], [R7], R8
    VMULL.U16       Q9, D9, D11
    VMLSL.U16       Q9, D12, D10
    VST1.32         D24[1], [R7], R8
    VST1.32         D25[0], [R7], R8
    VSHR.S32        Q9, Q9, #16
    VST1.32         D25[1], [R7], R8

    VMLAL.S16       Q9, D8, D11
    VMLSL.S16       Q9, D13, D10
    VQADD.S32       Q8, Q9, Q10
    VQSHL.S32       Q8, Q8, Q11

    VST1.32         D16[0], [R7], R8
    VST1.32         D16[1], [R7], R8
    VST1.32         D17[0], [R7], R8
    VST1.32         D17[1], [R7], R8

    VPOP            {d8 - d15}
    LDMFD           sp!, {R4-R12, R15}
