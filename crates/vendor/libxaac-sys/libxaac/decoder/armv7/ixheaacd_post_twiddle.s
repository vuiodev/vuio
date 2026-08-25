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
.global ixheaacd_post_twiddle_armv7

ixheaacd_post_twiddle_armv7:

    STMFD           sp!, {R4-R12}
    VPUSH           {d8 - d15}
    LDR             R4, [sp, #100]

ARM_PROLOGUE:

    CMP             R3, #0x400
    MOVW            R6, #7500
    ADD             R2, R2, R6
    BLT             NEXT
    MOV             R4, #50
    MOV             R5, #-50
    MOV             R6, #4
    VDUP.16         D10, R4

    B               NEXT1

NEXT:
    MOVW            R4, #0x192
    MOVW            R5, #0xfe6e
    MOV             R6, #32
    VDUP.16         D10, R4

NEXT1:
    LDR             R7, [R1], #4
    LDR             R8, [R1], #4
    LDR             R9, [R2]
    ADD             R2, R2, R6

    SMULWT          R11, R8, R9
    SMULWB          R10, R8, R9
    SMULWT          R12, R7, R9
    SMLAWB          R8, R7, R9, R11

    SUB             R10, R10, R12

    MVN             R8, R8
    ADD             R8, R8, #1

    SMLAWB          R9, R10, R5, R8
    SMLAWB          R11, R8, R4, R10

    LSL             R7, R3, #2
    ADD             R7, R0, R7
    SUB             R7, R7, #4

    STR             R11, [R7], #-4
    STR             R9, [R0], #4

    LSL             R5, R3, #2
    ADD             R5, R1, R5
    SUB             R5, R5, #40

    SUB             R3, R3, #1
    ASR             R3, R3, #4


    SUB             R7, R7, #28












    MOV             R8, #-32

NEON_PROLOGUE:

    VLD4.16         {D0, D1, D2, D3}, [R5], R8
    VLD4.16         {D4, D5, D6, D7}, [R1]!

    VLD2.16         {D8[0], D9[0]}, [R2], R6
    VLD2.16         {D8[1], D9[1]}, [R2], R6
    VLD2.16         {D8[2], D9[2]}, [R2], R6
    VLD2.16         {D8[3], D9[3]}, [R2], R6

    VREV64.16       Q6, Q4

    VMULL.U16       Q15, D2, D13
    VMULL.U16       Q14, D0, D13
    VMULL.U16       Q13, D2, D12
    VMULL.U16       Q12, D0, D12

    VSHR.U32        Q15, Q15, #16
    VSHR.U32        Q14, Q14, #16
    VSHR.U32        Q13, Q13, #16
    VSHR.U32        Q12, Q12, #16

    VMLAL.S16       Q15, D3, D13
    VMLAL.S16       Q14, D1, D13
    VMLAL.S16       Q13, D3, D12
    VMLAL.S16       Q12, D1, D12

    VMULL.U16       Q11, D6, D9
    VMULL.U16       Q10, D4, D9


    VADD.I32        Q14, Q14, Q13
    VSUB.I32        Q15, Q15, Q12
    VNEG.S32        Q14, Q14

    VMULL.U16       Q9, D6, D8
    VMULL.U16       Q8, D4, D8

    VMOV            Q13, Q15
    VSHR.U32        Q11, Q11, #16

    VMOV            Q12, Q14
    VSHR.U32        Q10, Q10, #16

    VUZP.16         D26, D27
    VSHR.U32        Q9, Q9, #16

    VUZP.16         D24, D25
    VSHR.U32        Q8, Q8, #16


    VMLAL.S16       Q11, D7, D9
    VMLAL.S16       Q10, D5, D9
    VMLAL.S16       Q9, D7, D8
    VMLAL.S16       Q8, D5, D8

    VLD2.16         {D8[0], D9[0]}, [R2], R6
    VMULL.U16       Q0, D26, D10

    VLD2.16         {D8[1], D9[1]}, [R2], R6
    VMULL.U16       Q1, D24, D10

    VLD2.16         {D8[2], D9[2]}, [R2], R6
    VADD.I32        Q11, Q11, Q8

    VLD2.16         {D8[3], D9[3]}, [R2], R6
    VSUB.I32        Q10, Q9, Q10

    VREV64.16       Q6, Q4
    VNEG.S32        Q11, Q11


    VMOV            Q9, Q11
    VSHR.U32        Q0, Q0, #16

    VMOV            Q8, Q10
    VSHR.U32        Q1, Q1, #16

    VUZP.16         D18, D19
    VMLAL.S16       Q0, D27, D10

    VUZP.16         D16, D17
    VMLAL.S16       Q1, D25, D10

    VMULL.U16       Q2, D18, D10
    VMULL.U16       Q3, D16, D10

    VNEG.S32        Q0, Q0
    VADD.I32        Q7, Q15, Q1
    VADD.I32        Q13, Q14, Q0

    VREV64.32       Q7, Q7
    VSHR.U32        Q2, Q2, #16

    VSWP            D14, D15
    VSHR.U32        Q3, Q3, #16

    VMLAL.S16       Q2, D19, D10
    VLD4.16         {D0, D1, D2, D3}, [R5], R8
    VMLAL.S16       Q3, D17, D10
    SUB             R3, R3, #2

    VADD.I32        Q12, Q10, Q2

    VREV64.32       Q12, Q12
    VNEG.S32        Q8, Q3

    VLD4.16         {D4, D5, D6, D7}, [R1]!

    VSWP            D24, D25
    VADD.I32        Q8, Q11, Q8




CORE_LOOP_PT:
    VMULL.U16       Q15, D2, D13
    VST2.32         {Q12, Q13}, [R7], R8
    VMULL.U16       Q14, D0, D13

    VMULL.U16       Q13, D2, D12
    VST2.32         {Q7, Q8}, [R0]!
    VMULL.U16       Q12, D0, D12

    VSHR.U32        Q15, Q15, #16
    VSHR.U32        Q14, Q14, #16
    VSHR.U32        Q13, Q13, #16
    VSHR.U32        Q12, Q12, #16

    VMLAL.S16       Q15, D3, D13
    VMLAL.S16       Q14, D1, D13
    VMLAL.S16       Q13, D3, D12
    VMLAL.S16       Q12, D1, D12

    VMULL.U16       Q11, D6, D9
    VMULL.U16       Q10, D4, D9


    VADD.I32        Q14, Q14, Q13
    VSUB.I32        Q15, Q15, Q12
    VNEG.S32        Q14, Q14

    VMULL.U16       Q9, D6, D8
    VMULL.U16       Q8, D4, D8


    VMOV            Q13, Q15
    VSHR.U32        Q11, Q11, #16

    VMOV            Q12, Q14
    VSHR.U32        Q10, Q10, #16

    VUZP.16         D26, D27
    VSHR.U32        Q9, Q9, #16

    VUZP.16         D24, D25
    VSHR.U32        Q8, Q8, #16


    VMLAL.S16       Q11, D7, D9
    VMLAL.S16       Q10, D5, D9
    VMLAL.S16       Q9, D7, D8
    VMLAL.S16       Q8, D5, D8

    VLD2.16         {D8[0], D9[0]}, [R2], R6
    VMULL.U16       Q0, D26, D10

    VLD2.16         {D8[1], D9[1]}, [R2], R6
    VMULL.U16       Q1, D24, D10

    VLD2.16         {D8[2], D9[2]}, [R2], R6
    VADD.I32        Q11, Q11, Q8

    VLD2.16         {D8[3], D9[3]}, [R2], R6
    VSUB.I32        Q10, Q9, Q10

    VREV64.16       Q6, Q4
    VNEG.S32        Q11, Q11


    VMOV            Q9, Q11
    VSHR.U32        Q0, Q0, #16

    VMOV            Q8, Q10
    VSHR.U32        Q1, Q1, #16

    VUZP.16         D18, D19
    VMLAL.S16       Q0, D27, D10

    VUZP.16         D16, D17
    VMLAL.S16       Q1, D25, D10

    VMULL.U16       Q2, D18, D10
    VMULL.U16       Q3, D16, D10

    VNEG.S32        Q0, Q0
    VADD.I32        Q7, Q15, Q1
    VADD.I32        Q13, Q14, Q0

    VREV64.32       Q7, Q7
    VSHR.U32        Q2, Q2, #16

    VSWP            D14, D15
    VSHR.U32        Q3, Q3, #16

    VMLAL.S16       Q2, D19, D10
    VLD4.16         {D0, D1, D2, D3}, [R5], R8
    VMLAL.S16       Q3, D17, D10

    VADD.I32        Q12, Q10, Q2
    VREV64.32       Q12, Q12
    VNEG.S32        Q8, Q3

    VLD4.16         {D4, D5, D6, D7}, [R1]!
    VSWP            D24, D25
    VADD.I32        Q8, Q11, Q8

    SUBS            R3, R3, #1
    BNE             CORE_LOOP_PT




NEON_EPILOGUE:
    VMULL.U16       Q15, D2, D13
    VST2.32         {Q12, Q13}, [R7], R8
    VMULL.U16       Q14, D0, D13

    VMULL.U16       Q13, D2, D12
    VST2.32         {Q7, Q8}, [R0]!
    VMULL.U16       Q12, D0, D12

    VSHR.U32        Q15, Q15, #16
    VSHR.U32        Q14, Q14, #16
    VSHR.U32        Q13, Q13, #16
    VSHR.U32        Q12, Q12, #16

    VMLAL.S16       Q15, D3, D13
    VMLAL.S16       Q14, D1, D13
    VMLAL.S16       Q13, D3, D12
    VMLAL.S16       Q12, D1, D12

    VMULL.U16       Q11, D6, D9
    VMULL.U16       Q10, D4, D9


    VADD.I32        Q14, Q14, Q13
    VSUB.I32        Q15, Q15, Q12
    VNEG.S32        Q14, Q14

    VMULL.U16       Q9, D6, D8
    VMULL.U16       Q8, D4, D8


    VMOV            Q13, Q15
    VSHR.U32        Q11, Q11, #16

    VMOV            Q12, Q14
    VSHR.U32        Q10, Q10, #16

    VUZP.16         D26, D27
    VSHR.U32        Q9, Q9, #16

    VUZP.16         D24, D25
    VSHR.U32        Q8, Q8, #16


    VMLAL.S16       Q11, D7, D9
    VMLAL.S16       Q10, D5, D9
    VMLAL.S16       Q9, D7, D8
    VMLAL.S16       Q8, D5, D8

    VMULL.U16       Q0, D26, D10
    VMULL.U16       Q1, D24, D10

    VADD.I32        Q11, Q11, Q8
    VSUB.I32        Q10, Q9, Q10
    VNEG.S32        Q11, Q11


    VMOV            Q9, Q11
    VSHR.U32        Q0, Q0, #16

    VMOV            Q8, Q10
    VSHR.U32        Q1, Q1, #16

    VUZP.16         D18, D19
    VMLAL.S16       Q0, D27, D10

    VUZP.16         D16, D17
    VMLAL.S16       Q1, D25, D10

    VMULL.U16       Q2, D18, D10
    VMULL.U16       Q3, D16, D10

    VNEG.S32        Q0, Q0
    VADD.I32        Q7, Q15, Q1
    VADD.I32        Q13, Q14, Q0

    VREV64.32       Q7, Q7
    VSHR.U32        Q2, Q2, #16

    VSWP            D14, D15
    VSHR.U32        Q3, Q3, #16

    VMLAL.S16       Q2, D19, D10
    VMLAL.S16       Q3, D17, D10

    VADD.I32        Q12, Q10, Q2

    VREV64.32       Q12, Q12
    VNEG.S32        Q8, Q3

    VSWP            D24, D25
    VADD.I32        Q8, Q11, Q8


    VST2.32         {Q7, Q8}, [R0]!
    VST2.32         {Q12, Q13}, [R7], R8



    VLD4.16         {D0, D1, D2, D3}, [R5], R8

    VMOV.S32        D5, #0x00000000
    VMOV.S32        D7, #0x00000000

    VLD2.32         {D4, D6}, [R1]!
    VLD2.32         {D5[0], D7[0]}, [R1]

    VLD2.16         {D8[0], D9[0]}, [R2], R6
    VLD2.16         {D8[1], D9[1]}, [R2], R6
    VLD2.16         {D8[2], D9[2]}, [R2], R6
    VLD2.16         {D8[3], D9[3]}, [R2], R6

    VREV64.16       Q6, Q4

    VUZP.16         D4, D5
    VUZP.16         D6, D7

    VMULL.U16       Q15, D2, D13
    VMULL.U16       Q14, D0, D13

    VMULL.U16       Q13, D2, D12
    VMULL.U16       Q12, D0, D12

    VSHR.U32        Q15, Q15, #16
    VSHR.U32        Q14, Q14, #16
    VSHR.U32        Q13, Q13, #16
    VSHR.U32        Q12, Q12, #16

    VMLAL.S16       Q15, D3, D13
    VMLAL.S16       Q14, D1, D13
    VMLAL.S16       Q13, D3, D12
    VMLAL.S16       Q12, D1, D12

    VMULL.U16       Q11, D6, D9
    VMULL.U16       Q10, D4, D9


    VADD.I32        Q14, Q14, Q13
    VSUB.I32        Q15, Q15, Q12
    VNEG.S32        Q14, Q14

    VMULL.U16       Q9, D6, D8
    VMULL.U16       Q8, D4, D8


    VMOV            Q13, Q15
    VSHR.U32        Q11, Q11, #16

    VMOV            Q12, Q14
    VSHR.U32        Q10, Q10, #16

    VUZP.16         D26, D27
    VSHR.U32        Q9, Q9, #16

    VUZP.16         D24, D25
    VSHR.U32        Q8, Q8, #16


    VMLAL.S16       Q11, D7, D9
    VMLAL.S16       Q10, D5, D9
    VMLAL.S16       Q9, D7, D8
    VMLAL.S16       Q8, D5, D8


    VMULL.U16       Q0, D26, D10
    VMULL.U16       Q1, D24, D10


    VADD.I32        Q11, Q11, Q8
    VSUB.I32        Q10, Q9, Q10
    VNEG.S32        Q11, Q11


    VMOV            Q9, Q11
    VSHR.U32        Q0, Q0, #16

    VMOV            Q8, Q10
    VSHR.U32        Q1, Q1, #16

    VUZP.16         D18, D19
    VMLAL.S16       Q0, D27, D10

    VUZP.16         D16, D17
    VMLAL.S16       Q1, D25, D10

    VMULL.U16       Q2, D18, D10
    VMULL.U16       Q3, D16, D10

    VNEG.S32        Q0, Q0
    VADD.I32        Q7, Q15, Q1
    VADD.I32        Q13, Q14, Q0

    VREV64.32       Q7, Q7
    VSHR.U32        Q2, Q2, #16

    VSWP            D14, D15
    VSHR.U32        Q3, Q3, #16

    VMLAL.S16       Q2, D19, D10

    VMLAL.S16       Q3, D17, D10

    VADD.I32        Q12, Q10, Q2

    VREV64.32       Q12, Q12
    VNEG.S32        Q8, Q3

    VSWP            D24, D25
    VADD.I32        Q8, Q11, Q8

    VST2.32         {D14, D16}, [R0]!
    VST2.32         {D15[0], D17[0]}, [R0]!
    VST1.32         D15[1], [R0]

    ADD             R7, R7, #4

    VST1.32         D26[0], [R7]!
    VST2.32         {D24[1], D26[1]}, [R7]!
    VST2.32         {D25, D27}, [R7]

    VPOP            {d8 - d15}
    LDMFD           sp!, {R4-R12}
    BX              LR
