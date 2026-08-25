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
.global ixheaacd_post_twid_overlap_add_armv7

ixheaacd_post_twid_overlap_add_armv7:

    STMFD           sp!, {R4-R12}
    VPUSH           {d8 - d15}

    LDR             R4, [sp, #100]
    LDR             R5, [sp, #104]
    LDR             R6, [sp, #108]

    LSL             R9, R3, #2
    ASR             R9, R9, #1
    ADD             R6, R6, R9
    SUB             R6, R6, #4

    MOVW            R8, #7500
    ADD             R2, R2, R8



    VMOV.S16        D18, #50
    RSB             R9, R5, #15
    VMOV.S32        Q10, #0x0000000
    VDUP.32         Q8, R5
    SUB             R5, R5, #16
    STR             R5, [sp, #116]
    MOV             R8, #1
    LSL             R8, R8, R9
    STR             R8, [sp, #120]


ARM_PROLOGUE:


    LDR             R8, [R1], #4
    LDR             R9, [R1], #4

    LDR             R10, [R2], #4

    SMULWT          R11, R8, R10
    SMULWB          R12, R9, R10
    SMULWB          R5, R8, R10
    SMLAWT          R7, R9, R10, R5
    SUB             R8, R12, R11
    MVN             R5, R7
    ADD             R5, R5, #1

    MOV             R9, #50
    MOV             R12, #-50
    SMULWB          R10, R5, R9
    SMULWB          R11, R8, R12

    ADD             R8, R8, R10
    ADD             R5, R5, R11

    LDR             R11, [sp, #104]
    LDR             R10, [R6], #-32


    SMULWB          R7, R8, R10
    MVN             R8, R8
    ADD             R8, R8, #1
    SMULWT          R12, R8, R10

    CMP             R11, #0
    BLT             NEXT

    RSBS            R9, R11, #16




    LDR             R8, [sp, #120]
    QADD            R5, R5, R8
    ASR             R5, R5, R9

    RSBS            R9, R11, #31
    MOVS            R8, R7, ASR R9
    CMNLT           R8, #1
    MOVLT           R7, #0x80000000
    MVNGT           R7, #0x80000000
    MOVEQ           R7, R7, LSL R11

    RSBS            R9, R11, #31
    MOVS            R8, R12, ASR R9
    CMNLT           R8, #1
    MOVLT           R12, #0x80000000
    MVNGT           R12, #0x80000000
    MOVEQ           R12, R12, LSL R11

    B               NEXT1
NEXT:
    MVN             R11, R11
    ADD             R11, R11, #1
    ASR             R5, R5, R11
    MOV             R8, #0x8000
    QADD            R5, R5, R8
    ASR             R5, R5, #16
    ASR             R7, R7, R11
    ASR             R12, R12, R11

NEXT1:
    LDR             R9, [R4]
    MOV             R8, #0x8000

    STR             R5, [R4], #4


    UXTH            R5, R10, ROR #16
    UXTH            R10, R10


    VDUP.32         D0, R9
    VDUP.32         D2, R10
    VDUP.32         D3, R5
    VZIP.32         D2, D3
    VMULL.S32       Q0, D2, D0
    VQMOVN.S64      D8, Q0


    VDUP.32         D0, R12
    VDUP.32         D1, R7

    VZIP.32         D0, D1
    VQSUB.S32       D8, D0, D8


    VDUP.32         D0, R8



    LDR             R7, [sp, #112]
    LSL             R7 , R7 , #1
    LSL             R10, R7, #1

    ASR             R5, R3, #1
    SMULBB          R5, R10, R5
    ADD             R5, R5, R0
    SUB             R0, R5, R10
    MVN             R9, R10
    ADD             R9, R9, #1

    VST1.32         D8[1], [R0], R9
    VST1.32         D8[0], [R5], R10


    MOV             R8, R1
    LSL             R12, R3, #2

    ADD             R1, R1, R12

    SUB             R1, R1, #40

    MOV             R12, #-32



PROLOGUE_NEON:

    ASR             R3, R3, #2
    SUB             R3, R3, #4
    ASR             R3, R3, #2
    SUB             R3, R3, #2

    VLD2.32         {Q0, Q1}, [R1], R12
    VUZP.16         D0, D1
    VUZP.16         D2, D3

    VREV64.16       Q0, Q0
    VREV64.16       Q1, Q1
    VLD2.16         {D8, D9}, [R2]!

    VLD2.32         {Q2, Q3}, [R8]!
    VMULL.U16       Q15, D0, D9

    VUZP.16         D4, D5
    VMULL.U16       Q14, D2, D8

    VUZP.16         D6, D7
    VMULL.U16       Q13, D0, D8


    VMULL.U16       Q12, D2, D9

    VLD2.32         {Q5, Q6}, [R6], R12
    VSHR.U32        Q15, Q15, #16

    VUZP.16         D10, D11
    VSHR.U32        Q14, Q14, #16

    VUZP.16         D12, D13
    VMLAL.S16       Q15, D1, D9

    VREV64.16       Q5, Q5
    VMLAL.S16       Q14, D3, D8

    VREV64.16       Q6, Q6
    VSHR.U32        Q13, Q13, #16


    VSHR.U32        Q12, Q12, #16

    VMLAL.S16       Q13, D1, D8
    VMLAL.S16       Q12, D3, D9



    VADD.I32        Q15, Q15, Q14
    VNEG.S32        Q15, Q15

    VMULL.U16       Q11, D4, D8

    VSUB.I32        Q14, Q12, Q13


    VMOV            Q13, Q15
    VMOV            Q12, Q14

    VUZP.16         D24, D25


    VUZP.16         D26, D27
    VMULL.U16       Q1, D24, D18

    VMULL.U16       Q0, D26, D18

    VSHR.U32        Q11, Q11, #16
    VMLAL.S16       Q11, D5, D8

    VSHR.U32        Q1, Q1, #16
    VSHR.U32        Q0, Q0, #16
    VMLAL.S16       Q1, D25, D18
    VMLAL.S16       Q0, D27, D18

    VMULL.U16       Q12, D4, D9
    VMULL.U16       Q13, D6, D8

    VNEG.S32        Q1, Q1
    VADD.I32        Q14, Q14, Q0
    VADD.I32        Q15, Q15, Q1

    VMULL.U16       Q0, D6, D9
    VSHR.S32        Q12, Q12, #16
    VMLAL.S16       Q12, D5, D9
    VSHR.S32        Q13, Q13, #16
    VSHR.S32        Q0, Q0, #16
    VMLAL.S16       Q13, D7, D8
    VMLAL.S16       Q0, D7, D9




    VADD.I32        Q11, Q11, Q0
    VNEG.S32        Q11, Q11
    VSUB.I32        Q12, Q13, Q12



    LDR             R11, [sp, #120]
    VDUP.S32        Q7, R11
    VQADD.S32       Q14, Q14, Q7
    LDR             R11, [sp, #116]
    VDUP.S32        Q0, R11
    VQSHL.S32       Q14, Q14, Q0

    VMOV            Q0, Q11
    VMOV            Q7, Q12


    VUZP.16         D24, D25

    VUZP.16         D22, D23
    VMULL.U16       Q4, D24, D18
    VMULL.U16       Q13, D22, D18

    VNEG.S32        Q1, Q15
    VUZP.16         D30, D31

    VUZP.16         D2, D3
    VMULL.U16       Q2, D30, D12

    VMULL.U16       Q3, D2, D13

    VSHR.U32        Q4, Q4, #16
    VSHR.U32        Q13, Q13, #16

    VMLAL.S16       Q4, D25, D18
    VMLAL.S16       Q13, D23, D18

    VSHR.U32        Q2, Q2, #16
    VSHR.U32        Q3, Q3, #16

    VMLAL.S16       Q2, D31, D12
    VMLAL.S16       Q3, D3, D13

    VNEG.S32        Q4, Q4
    VADD.I32        Q7, Q7, Q13
    VADD.I32        Q0, Q0, Q4

    LDR             R11, [sp, #120]
    VDUP.S32        Q4, R11
    VQADD.S32       Q0, Q0, Q4
    LDR             R11, [sp, #116]
    VDUP.S32        Q13, R11
    VQSHL.S32       Q0, Q0, Q13

    VMOV            Q13, Q14

    VLD2.32         {Q14, Q15}, [R4]

    VZIP.32         Q13, Q0
    VST1.32         {Q13}, [R4]!
    VST1.32         {Q0}, [R4]!

    VMOV.S32        D1, #0
    VADDL.S16       Q0, D13, D1


    VMULL.S32       Q13, D28, D0
    VQMOVN.S64      D8, Q13
    VMULL.S32       Q13, D29, D1
    VQMOVN.S64      D9, Q13

    VMOV.S32        D1, #0
    VADDL.S16       Q0, D12, D1


    VMULL.S32       Q12, D28, D0
    VQMOVN.S64      D26, Q12
    VMULL.S32       Q12, D29, D1
    VQMOVN.S64      D27, Q12

    VQSHL.S32       Q2, Q2, Q8
    VQSHL.S32       Q3, Q3, Q8

    VQSUB.S32       Q2, Q2, Q4
    VQSUB.S32       Q3, Q3, Q13

    VNEG.S32        Q13, Q7
    VUZP.16         D14, D15
    VUZP.16         D26, D27

    VMOV.S32        D1, #0
    VADDL.S16       Q0, D10, D1
    VMULL.S32       Q11, D30, D0
    VQMOVN.S64      D24, Q11
    VMULL.S32       Q11, D31, D1
    VQMOVN.S64      D25, Q11
    VMOV.S32        D1, #0
    VADDL.S16       Q0, D11, D1
    VMULL.S32       Q4, D30, D0
    VQMOVN.S64      D22, Q4
    VMULL.S32       Q4, D31, D1
    VQMOVN.S64      D23, Q4
    VMULL.U16       Q4, D26, D11
    VMULL.U16       Q15, D14, D10

    VLD2.32         {Q0, Q1}, [R1], R12

    VUZP.16         D0, D1

    VUZP.16         D2, D3
    VSHR.U32        Q4, Q4, #16

    VREV64.16       Q0, Q0
    VSHR.U32        Q15, Q15, #16

    VREV64.16       Q1, Q1
    VMLAL.S16       Q4, D27, D11

    VMLAL.S16       Q15, D15, D10

    VLD2.32         {Q5, Q6}, [R6], R12

    VUZP.16         D10, D11

    VUZP.16         D12, D13
    VMOV            Q7,Q2

    VREV64.16       Q5, Q5

    VREV64.16       Q6, Q6
    VQSHL.S32       Q4, Q4, Q8

    VMOV             Q9,Q3

    VLD2.32         {Q2, Q3}, [R8]!
    VQSHL.S32       Q15, Q15, Q8

    VUZP.16         D4, D5
    VQSUB.S32       Q4, Q4, Q12

    VUZP.16         D6, D7
    VQSUB.S32       Q11, Q15, Q11

     VMOV            Q15,Q4

    VLD2.16         {D8, D9}, [R2]!

CORE_LOOP_PTO:

    VST1.32         D14[0], [R0, :32], R9

    VST1.32         D22[0], [R0, :32], R9

    VST1.32         D14[1], [R0, :32], R9

    VST1.32         D22[1], [R0, :32], R9

    VST1.32         D15[0], [R0, :32], R9

    VST1.32         D23[0], [R0, :32], R9

    VST1.32         D15[1], [R0, :32], R9

    VST1.32         D23[1], [R0, :32], R9

    VST1.32         D18[0], [R5, :32], R10

    VST1.32         D30[0], [R5, :32] ,R10

    VST1.32         D18[1], [R5, :32], R10

    VST1.32         D30[1], [R5, :32], R10

    VST1.32         D19[0], [R5, :32], R10

    VST1.32         D31[0], [R5, :32], R10

    VST1.32         D19[1], [R5, :32], R10

    VST1.32         D31[1], [R5, :32], R10

    VMULL.U16       Q15, D0, D9
    VMULL.U16       Q14, D2, D8
    VMULL.U16       Q13, D0, D8
    VMULL.U16       Q12, D2, D9
    VSHR.U32        Q15, Q15, #16
    VSHR.U32        Q14, Q14, #16
    VMLAL.S16       Q15, D1, D9
    VMLAL.S16       Q14, D3, D8
    VSHR.U32        Q13, Q13, #16
    VSHR.U32        Q12, Q12, #16
    VMLAL.S16       Q13, D1, D8
    VMLAL.S16       Q12, D3, D9

    VADD.I32        Q15, Q15, Q14
    VNEG.S32        Q15, Q15
    VSUB.I32        Q14, Q12, Q13

    VMOV            Q13, Q15
    VMULL.U16       Q11, D4, D8

    VMOV            Q12, Q14

    VUZP.16         D24, D25

    VMOV.S16        D18, #50

    VUZP.16         D26, D27
    VMULL.U16       Q1, D24, D18
    VMULL.U16       Q0, D26, D18

    VSHR.U32        Q11, Q11, #16
    VMLAL.S16       Q11, D5, D8

    VSHR.U32        Q1, Q1, #16
    VSHR.U32        Q0, Q0, #16
    VMLAL.S16       Q1, D25, D18
    VMLAL.S16       Q0, D27, D18

    VMULL.U16       Q12, D4, D9
    VMULL.U16       Q13, D6, D8

    VNEG.S32        Q1, Q1
    VADD.I32        Q14, Q14, Q0
    VADD.I32        Q15, Q15, Q1

    VMULL.U16       Q0, D6, D9
    VSHR.S32        Q12, Q12, #16
    VMLAL.S16       Q12, D5, D9
    VSHR.S32        Q13, Q13, #16
    VSHR.S32        Q0, Q0, #16
    VMLAL.S16       Q13, D7, D8
    VMLAL.S16       Q0, D7, D9



    VADD.I32        Q11, Q11, Q0

    VNEG.S32        Q11, Q11
    VSUB.I32        Q12, Q13, Q12


    LDR             R11, [sp, #120]
    VDUP.S32        Q7, R11
    VQADD.S32       Q14, Q14, Q7
    LDR             R11, [sp, #116]
    VDUP.S32        Q0, R11
    VQSHL.S32       Q14, Q14, Q0


    VMOV            Q0, Q11
    VMOV            Q7, Q12

    VUZP.16         D24, D25

    VUZP.16         D22, D23
    VMULL.U16       Q4, D24, D18
    VMULL.U16       Q13, D22, D18

    VNEG.S32        Q1, Q15

    VUZP.16         D30, D31

    VUZP.16         D2, D3
    VMULL.U16       Q2, D30, D12
    VMULL.U16       Q3, D2, D13

    VSHR.U32        Q4, Q4, #16
    VSHR.U32        Q13, Q13, #16

    VMLAL.S16       Q4, D25, D18
    VMLAL.S16       Q13, D23, D18

    VSHR.U32        Q2, Q2, #16
    VSHR.U32        Q3, Q3, #16

    VMLAL.S16       Q2, D31, D12
    VMLAL.S16       Q3, D3, D13

    VNEG.S32        Q4, Q4
    VADD.I32        Q7, Q7, Q13
    VADD.I32        Q0, Q0, Q4



    LDR             R11, [sp, #120]
    VDUP.S32        Q4, R11
    VQADD.S32       Q0, Q0, Q4
    LDR             R11, [sp, #116]
    VDUP.S32        Q13, R11
    VQSHL.S32       Q0, Q0, Q13
    VMOV            Q13, Q14

    VLD2.32         {Q14, Q15}, [R4]

    VZIP.32         Q13, Q0
    VST1.32         {Q13}, [R4]!
    VST1.32         {Q0}, [R4]!

    VMOV.S32        D1, #0
    VADDL.S16       Q0, D13, D1


    VMULL.S32       Q13, D28, D0
    VQMOVN.S64      D8, Q13
    VMULL.S32       Q13, D29, D1
    VQMOVN.S64      D9, Q13

    VMOV.S32        D1, #0
    VADDL.S16       Q0, D12, D1


    VMULL.S32       Q12, D28, D0
    VQMOVN.S64      D26, Q12
    VMULL.S32       Q12, D29, D1
    VQMOVN.S64      D27, Q12

    VQSHL.S32       Q2, Q2, Q8
    VQSHL.S32       Q3, Q3, Q8



    VQSUB.S32       Q2, Q2, Q4
    VQSUB.S32       Q3, Q3, Q13

    VNEG.S32        Q13, Q7
    VUZP.16         D26, D27

    VMOV.S32        D1, #0
    VADDL.S16       Q0, D10, D1
    VMULL.S32       Q11, D30, D0
    VQMOVN.S64      D24, Q11
    VMULL.S32       Q11, D31, D1
    VQMOVN.S64      D25, Q11

    VMOV.S32        D1, #0
    VADDL.S16       Q0, D11, D1

    VMULL.S32       Q4, D30, D0
    VQMOVN.S64      D22, Q4
    VMULL.S32       Q4, D31, D1
    VQMOVN.S64      D23, Q4


    VUZP.16         D14, D15
    VMULL.U16       Q4, D26, D11
    VMULL.U16       Q15, D14, D10


    VLD2.32         {Q0, Q1}, [R1], R12

    VUZP.16         D0, D1

    VUZP.16         D2, D3
    VSHR.U32        Q4, Q4, #16

    VREV64.16       Q0, Q0
    VSHR.U32        Q15, Q15, #16

    VREV64.16       Q1, Q1
    VMLAL.S16       Q4, D27, D11

    VMLAL.S16       Q15, D15, D10

    VLD2.32         {Q5, Q6}, [R6], R12

    VUZP.16         D10, D11

    VUZP.16         D12, D13
    VMOV             Q7 , Q2

    VREV64.16       Q5, Q5

    VREV64.16       Q6, Q6


    VQSHL.S32       Q4, Q4, Q8

    VMOV            Q9,Q3

    VLD2.32         {Q2, Q3}, [R8]!
    VQSHL.S32       Q15, Q15, Q8

    VUZP.16         D4, D5
    VQSUB.S32       Q4, Q4, Q12

    VUZP.16         D6, D7
    VQSUB.S32       Q11, Q15, Q11

     VMOV            Q15,Q4

    VLD2.16         {D8, D9}, [R2]!

    SUBS            R3, R3, #1
    BNE             CORE_LOOP_PTO





EPILOGUE:

    VST1.32         D14[0], [R0], R9

    VST1.32         D22[0], [R0], R9

    VST1.32         D14[1], [R0], R9

    VST1.32         D22[1], [R0], R9

    VST1.32         D15[0], [R0], R9

    VST1.32         D23[0], [R0], R9

    VST1.32         D15[1], [R0], R9

    VST1.32         D23[1], [R0], R9

    VST1.32         D18[0], [R5], R10

    VST1.32         D30[0], [R5], R10

    VST1.32         D18[1], [R5], R10

    VST1.32         D30[1], [R5], R10

    VST1.32         D19[0], [R5], R10

    VST1.32         D31[0], [R5], R10

    VST1.32         D19[1], [R5], R10

    VST1.32         D31[1], [R5], R10

    VMULL.U16       Q15, D0, D9
    VMULL.U16       Q14, D2, D8
    VMULL.U16       Q13, D0, D8
    VMULL.U16       Q12, D2, D9
    VSHR.U32        Q15, Q15, #16
    VSHR.U32        Q14, Q14, #16
    VMLAL.S16       Q15, D1, D9
    VMLAL.S16       Q14, D3, D8
    VSHR.U32        Q13, Q13, #16
    VSHR.U32        Q12, Q12, #16
    VMLAL.S16       Q13, D1, D8
	VMLAL.S16       Q12, D3, D9
    VADD.I32        Q15, Q15, Q14
	VNEG.S32        Q15, Q15


    VSUB.I32        Q14, Q12, Q13

    VMOV.S16        D18, #50

    VMULL.U16       Q11, D4, D8
    VMOV            Q13, Q15
    VMOV            Q12, Q14

    VMOV            Q13, Q15
    VMOV            Q12, Q14

    VUZP.16         D26, D27
    VUZP.16         D24, D25


    VMULL.U16       Q1, D24, D18
    VMULL.U16       Q0, D26, D18

    VSHR.U32        Q11, Q11, #16
    VMLAL.S16       Q11, D5, D8

    VSHR.U32        Q1, Q1, #16
    VSHR.U32        Q0, Q0, #16
    VMLAL.S16       Q1, D25, D18
    VMLAL.S16       Q0, D27, D18

    VMULL.U16       Q12, D4, D9
    VMULL.U16       Q13, D6, D8

    VNEG.S32        Q1, Q1
    VADD.I32        Q14, Q14, Q0
    VADD.I32        Q15, Q15, Q1

    VMULL.U16       Q0, D6, D9
    VSHR.S32        Q12, Q12, #16
    VMLAL.S16       Q12, D5, D9
    VSHR.S32        Q13, Q13, #16
    VSHR.S32        Q0, Q0, #16
    VMLAL.S16       Q13, D7, D8
    VMLAL.S16       Q0, D7, D9





    VADD.I32        Q11, Q11, Q0
    VNEG.S32        Q11, Q11
    VSUB.I32        Q12, Q13, Q12




    LDR             R11, [sp, #120]
    VDUP.S32        Q7, R11
    VQADD.S32       Q14, Q14, Q7
    LDR             R11, [sp, #116]
    VDUP.S32        Q0, R11
    VQSHL.S32       Q14, Q14, Q0


    VMOV            Q0, Q11
    VMOV            Q7, Q12


    VUZP.16         D22, D23
    VUZP.16         D24, D25

    VMULL.U16       Q4, D24, D18
    VMULL.U16       Q13, D22, D18

    VNEG.S32        Q1, Q15
    VUZP.16         D30, D31
    VUZP.16         D2, D3

    VMULL.U16       Q2, D30, D12
    VMULL.U16       Q3, D2, D13

    VSHR.U32        Q4, Q4, #16
    VSHR.U32        Q13, Q13, #16

    VMLAL.S16       Q4, D25, D18
    VMLAL.S16       Q13, D23, D18

    VSHR.U32        Q2, Q2, #16
    VSHR.U32        Q3, Q3, #16

    VMLAL.S16       Q2, D31, D12
    VMLAL.S16       Q3, D3, D13

    VNEG.S32        Q4, Q4
    VADD.I32        Q7, Q7, Q13
    VADD.I32        Q0, Q0, Q4

    LDR             R11, [sp, #120]
    VDUP.S32        Q4, R11
    VQADD.S32       Q0, Q0, Q4
    LDR             R11, [sp, #116]
    VDUP.S32        Q13, R11
    VQSHL.S32       Q0, Q0, Q13


    VMOV            Q13, Q14

    VLD2.32         {Q14, Q15}, [R4]
    VZIP.32         Q13, Q0
    VST1.32         {Q13}, [R4]!
    VST1.32         {Q0}, [R4]!

    VMOV.S32        D1, #0
    VADDL.S16       Q0, D13, D1

    VMULL.S32       Q13, D28, D0
    VQMOVN.S64      D8, Q13
    VMULL.S32       Q13, D29, D1
    VQMOVN.S64      D9, Q13

    VMOV.S32        D1, #0
    VADDL.S16       Q0, D12, D1

    VMULL.S32       Q12, D28, D0
    VQMOVN.S64      D26, Q12
    VMULL.S32       Q12, D29, D1
    VQMOVN.S64      D27, Q12

    VQSHL.S32       Q2, Q2, Q8
    VQSHL.S32       Q3, Q3, Q8

    VQSUB.S32       Q2, Q2, Q4
    VQSUB.S32       Q3, Q3, Q13

    VNEG.S32        Q13, Q7
    VUZP.16         D14, D15
    VUZP.16         D26, D27

    VMOV.S32        D1, #0
    VADDL.S16       Q0, D10, D1


    VMULL.S32       Q11, D30, D0
    VQMOVN.S64      D24, Q11
    VMULL.S32       Q11, D31, D1
    VQMOVN.S64      D25, Q11

    VMOV.S32        D1, #0
    VADDL.S16       Q0, D11, D1

    VMULL.S32       Q4, D30, D0
    VQMOVN.S64      D22, Q4
    VMULL.S32       Q4, D31, D1
    VQMOVN.S64      D23, Q4


    VMULL.U16       Q4, D26, D11
    VMULL.U16       Q15, D14, D10

    VSHR.U32        Q4, Q4, #16

    VSHR.U32        Q15, Q15, #16

    VMLAL.S16       Q4, D27, D11

    VMLAL.S16       Q15, D15, D10



    VQADD.S32       Q7, Q2, Q10

    VQSHL.S32       Q4, Q4, Q8

    VQSHL.S32       Q15, Q15, Q8

    VQSUB.S32       Q4, Q4, Q12

    VQSUB.S32       Q11, Q15, Q11

    VMOV             Q15 , Q4

    VST1.32         D14[0], [R0], R9
    VST1.32         D22[0], [R0], R9
    VST1.32         D14[1], [R0], R9
    VST1.32         D22[1], [R0], R9
    VST1.32         D15[0], [R0], R9
    VST1.32         D23[0], [R0], R9
    VST1.32         D15[1], [R0], R9
    VST1.32         D23[1], [R0], R9
    VST1.32         D6[0], [R5], R10
    VST1.32         D30[0], [R5], R10
    VST1.32         D6[1], [R5], R10
    VST1.32         D30[1], [R5], R10
    VST1.32         D7[0], [R5], R10
    VST1.32         D31[0], [R5], R10
    VST1.32         D7[1], [R5], R10
    VST1.32         D31[1], [R5], R10

ARM_EPILOGUE:

ARM_LOOP:

    VLD2.32         {Q0, Q1}, [R1]

    VUZP.16         D0, D1
    VUZP.16         D2, D3

    VREV64.16       Q0, Q0
    VREV64.16       Q1, Q1

    VLD2.16         {D8, D9}, [R2]!

    VLD2.32         {D4, D6}, [R8]!
    VMOV.S32        D5, #0x00000000
    VMOV.S32        D7, #0x00000000

    VLD1.32         D5[0], [R8]!
    VLD1.32         D7[0], [R8]

    MOV             R12, #16

    VUZP.16         D4, D5
    VUZP.16         D6, D7

    ADD             R6, R6, #16

    MOV             R12, #-4
    VLD2.32         {D11, D13}, [R6], R12


    VMOV.S32        D10, #0x00000000

    VLD1.32         D12[1], [R6], R12
    VLD1.32         D10[1], [R6], R12
    VLD1.32         D12[0], [R6], R12

    VUZP.16         D10, D11
    VUZP.16         D12, D13

    VREV64.16       Q5, Q5
    VREV64.16       Q6, Q6

    VMULL.U16       Q15, D0, D9
    VMULL.U16       Q14, D2, D8
    VMULL.U16       Q13, D0, D8
    VMULL.U16       Q12, D2, D9

    VSHR.U32        Q15, Q15, #16
    VSHR.U32        Q14, Q14, #16

    VMLAL.S16       Q15, D1, D9
    VMLAL.S16       Q14, D3, D8

    VSHR.U32        Q13, Q13, #16
    VSHR.U32        Q12, Q12, #16

    VMLAL.S16       Q13, D1, D8
    VMLAL.S16       Q12, D3, D9

    VADD.I32        Q15, Q15, Q14
    VNEG.S32        Q15, Q15

    VMULL.U16       Q11, D4, D8

    VSUB.I32        Q14, Q12, Q13


    VMOV            Q13, Q15
    VMOV            Q12, Q14

    VUZP.16         D26, D27
    VUZP.16         D24, D25


    VMULL.U16       Q1, D24, D18
    VMULL.U16       Q0, D26, D18

    VSHR.U32        Q11, Q11, #16
    VMLAL.S16       Q11, D5, D8

    VSHR.U32        Q1, Q1, #16
    VSHR.U32        Q0, Q0, #16
    VMLAL.S16       Q1, D25, D18
    VMLAL.S16       Q0, D27, D18

    VMULL.U16       Q12, D4, D9
    VMULL.U16       Q13, D6, D8

    VNEG.S32        Q1, Q1
    VADD.I32        Q14, Q14, Q0
    VADD.I32        Q15, Q15, Q1

    VMULL.U16       Q0, D6, D9
    VSHR.S32        Q12, Q12, #16
    VMLAL.S16       Q12, D5, D9
    VSHR.S32        Q13, Q13, #16
    VSHR.S32        Q0, Q0, #16
    VMLAL.S16       Q13, D7, D8
    VMLAL.S16       Q0, D7, D9

    VADD.I32        Q11, Q11, Q0
    VNEG.S32        Q11, Q11
    VSUB.I32        Q12, Q13, Q12

    LDR             R11, [sp, #120]
    VDUP.S32        Q7, R11
    VQADD.S32       Q14, Q14, Q7
    LDR             R11, [sp, #116]
    VDUP.S32        Q0, R11
    VQSHL.S32       Q14, Q14, Q0

    VMOV            Q0, Q11
    VMOV            Q7, Q12

    VUZP.16         D22, D23
    VUZP.16         D24, D25

    VMULL.U16       Q4, D24, D18
    VMULL.U16       Q13, D22, D18

    VNEG.S32        Q1, Q15
    VUZP.16         D30, D31
    VUZP.16         D2, D3

    VMULL.U16       Q2, D30, D12
    VMULL.U16       Q3, D2, D13

    VSHR.U32        Q4, Q4, #16
    VSHR.U32        Q13, Q13, #16

    VMLAL.S16       Q4, D25, D18
    VMLAL.S16       Q13, D23, D18

    VSHR.U32        Q2, Q2, #16
    VSHR.U32        Q3, Q3, #16

    VMLAL.S16       Q2, D31, D12
    VMLAL.S16       Q3, D3, D13

    VNEG.S32        Q4, Q4
    VADD.I32        Q7, Q7, Q13
    VADD.I32        Q0, Q0, Q4

    LDR             R11, [sp, #120]
    VDUP.S32        Q4, R11
    VQADD.S32       Q0, Q0, Q4
    LDR             R11, [sp, #116]
    VDUP.S32        Q13, R11
    VQSHL.S32       Q0, Q0, Q13

    VMOV            Q13, Q14

    MOV             R6, R4

    VLD1.32         {D28, D29}, [R4]!
    VMOV.S32        D31, #0x00000000
    VLD1.32         D30[0], [R4]!
    VLD1.32         D30[1], [R4]!
    VLD1.32         D31[0], [R4]!
    VUZP.32         Q14, Q15


    VST1.32         D26[0], [R6]!
    VST1.32         D0[0], [R6]!
    VST1.32         D26[1], [R6]!
    VST1.32         D0[1], [R6]!
    VST1.32         D27[0], [R6]!
    VST1.32         D1[0], [R6]!
    VST1.32         D27[1], [R6]!

    VMOV.S32        D1, #0
    VADDL.S16       Q0, D13, D1

    VMULL.S32       Q13, D28, D0
    VQMOVN.S64      D8, Q13
    VMULL.S32       Q13, D29, D1
    VQMOVN.S64      D9, Q13

    VMOV.S32        D1, #0
    VADDL.S16       Q0, D12, D1

    VMULL.S32       Q12, D28, D0
    VQMOVN.S64      D26, Q12
    VMULL.S32       Q12, D29, D1
    VQMOVN.S64      D27, Q12

    VQSHL.S32       Q2, Q2, Q8
    VQSHL.S32       Q3, Q3, Q8

    VQSUB.S32       Q2, Q2, Q4
    VQSUB.S32       Q3, Q3, Q13

    VNEG.S32        Q13, Q7
    VUZP.16         D14, D15
    VUZP.16         D26, D27

    VMOV.S32        D1, #0
    VADDL.S16       Q0, D10, D1


    VMULL.S32       Q11, D30, D0
    VQMOVN.S64      D24, Q11
    VMULL.S32       Q11, D31, D1
    VQMOVN.S64      D25, Q11


    VMOV.S32        D1, #0
    VADDL.S16       Q0, D11, D1

    VMULL.S32       Q4, D30, D0
    VQMOVN.S64      D22, Q4
    VMULL.S32       Q4, D31, D1
    VQMOVN.S64      D23, Q4


    VMULL.U16       Q4, D26, D11
    VMULL.U16       Q15, D14, D10

    VSHR.U32        Q4, Q4, #16

    VSHR.U32        Q15, Q15, #16

    VMLAL.S16       Q4, D27, D11

    VMLAL.S16       Q15, D15, D10



    VMOV           Q7 , Q2

    VQSHL.S32       Q4, Q4, Q8

    VQSHL.S32       Q15, Q15, Q8

    VQSUB.S32       Q4, Q4, Q12

    VQSUB.S32       Q11, Q15, Q11

     VMOV            Q15, Q4




    VST1.32         D14[0], [R0], R9
    VST1.32         D22[0], [R0], R9
    VST1.32         D14[1], [R0], R9
    VST1.32         D22[1], [R0], R9
    VST1.32         D15[0], [R0], R9
    VST1.32         D23[0], [R0], R9
    VST1.32         D15[1], [R0], R9

    VST1.32         D6[0], [R5], R10
    VST1.32         D30[0], [R5], R10
    VST1.32         D6[1], [R5], R10
    VST1.32         D30[1], [R5], R10
    VST1.32         D7[0], [R5], R10
    VST1.32         D31[0], [R5], R10
    VST1.32         D7[1], [R5], R10

    VPOP            {d8 - d15}
    LDMFD           sp!, {R4-R12}
    BX              LR


