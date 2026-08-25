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
.global ixheaacd_dct3_32
.type ixheaacd_dct3_32, %function
.extern ixheaacd_radix4bfly
.hidden ixheaacd_radix4bfly
.extern ixheaacd_postradixcompute4
.hidden ixheaacd_postradixcompute4

ixheaacd_dct3_32:


    STMFD           sp!, {R4-R12, R14}
    VPUSH           {D8 - D15}
    ADD             R6, R0, #196
    SUB             R7, R6, #8
    ADD             R10, R7, #4
    MOV             R9, #0
    VDUP.32         D0, R9
    ADD             R4, R2, #8
    MOV             R8, R1

    VLD1.32         D0[0], [R10]
    MOV             R11, #-4

    VSHR.S32        D0, D0, #7
    VLD4.16         {D12, D13, D14, D15}, [R4]!

    MOV             R12, #-16
    VST1.32         D0, [R8]!
    SUB             R7, R7, #12

    VLD1.32         {Q0}, [R6]!
    VLD1.32         {Q1}, [R7], R12
    SUB             R9, R6, #144

    VREV64.32       Q1, Q1
    SUB             R5, R7, #112

    VSWP            D2, D3
    VSHR.S32        Q0, Q0, #7

    VSHR.S32        Q1, Q1, #7
    VLD1.32         {Q3}, [R9]!
    VADD.I32        Q2, Q1, Q0

    VUZP.16         D4, D5
    VSHR.S32        Q3, Q3, #7


    VLD1.32         {Q4}, [R5], R12
    VMULL.U16       Q15, D4, D12

    VREV64.32       Q4, Q4
    VMULL.U16       Q14, D4, D13

    VSWP            D8, D9

    VSHR.S32        Q4, Q4, #7
    VLD1.32         {Q0}, [R6]!
    VSUB.I32        Q5, Q3, Q4

    VUZP.16         D10, D11

    VMLAL.U16       Q15, D10, D13
    VLD1.32         {Q1}, [R7], R12
    VMLSL.U16       Q14, D10, D12

    VREV64.32       Q1, Q1
    VSHR.S32        Q0, Q0, #7

    VSWP            D2, D3

    VSHR.U32        Q15, Q15, #16
    VSHR.S32        Q1, Q1, #7

    VMLAL.S16       Q15, D5, D12
    VMLAL.S16       Q15, D11, D13


    VSHR.S32        Q14, Q14, #16
    VMLAL.S16       Q14, D5, D13
    VADD.I32        Q2, Q1, Q0
    VMLSL.S16       Q14, D11, D12

    VUZP.16         D4, D5
    SUB             R9, R6, #144

    VSWP            Q15, Q14
    SUB             R5, R7, #112

    VST2.32         {Q14, Q15}, [R8]!

    VLD1.32         {Q3}, [R9]!

    VLD1.32         {Q4}, [R5], R12
    VSHR.S32        Q3, Q3, #7

    VREV64.32       Q4, Q4

    VSWP            D8, D9

    VSHR.S32        Q4, Q4, #7
    VLD4.16         {D12, D13, D14, D15}, [R4]!
    VSUB.I32        Q5, Q3, Q4

    VUZP.16         D10, D11
    VMULL.U16       Q15, D4, D12
    VMLAL.U16       Q15, D10, D13
    VMULL.U16       Q14, D4, D13
    VLD1.32         {Q0}, [R6]!
    VMLSL.U16       Q14, D10, D12
    VLD1.32         {Q1}, [R7], R11
    VSHR.U32        Q15, Q15, #16

    VREV64.32       Q1, Q1
    VSHR.S32        Q14, Q14, #16

    VSWP            D2, D3
    VSHR.S32        Q0, Q0, #7

    SUB             R9, R6, #144

    SUB             R5, R7, #124
    VLD1.32         {Q3}, [R9]!
    VMLAL.S16       Q14, D5, D13

    VMLSL.S16       Q14, D11, D12
    VLD1.32         {Q4}, [R5], R11
    VMLAL.S16       Q15, D5, D12

    VREV64.32       Q4, Q4
    VMLAL.S16       Q15, D11, D13

    VSWP            D8, D9
    VSHR.S32        Q1, Q1, #7

    VADD.I32        Q2, Q1, Q0
    VLD4.16         {D12, D13, D14, D15}, [R4]!
    VSHR.S32        Q3, Q3, #7

    VUZP.16         D4, D5
    VSHR.S32        Q4, Q4, #7

    VSWP            Q15, Q14
    VSUB.I32        Q5, Q3, Q4

    VST2.32         {Q14, Q15}, [R8]!

    VUZP.16         D10, D11
    VMULL.U16       Q15, D4, D12

    VMLAL.U16       Q15, D10, D13
    VLD1.32         D0, [R6]!
    VMULL.U16       Q14, D4, D13

    VMLSL.U16       Q14, D10, D12
    VLD1.32         D1[0], [R6]!

    VSHR.U32        Q15, Q15, #16
    VLD1.32         D2[0], [R7], R11

    VMLAL.S16       Q15, D5, D12
    VLD1.32         D2[1], [R7], R11
    VMLAL.S16       Q15, D11, D13

    SUB             R9, R6, #140
    VLD1.32         D3[0], [R7], R11

    SUB             R5, R7, #116
    VLD1.32         D6, [R9]!
    VSHR.S32        Q14, Q14, #16

    VSHR.S32        Q0, Q0, #7
    VLD1.32         D7[0], [R9]!
    VMLAL.S16       Q14, D5, D13

    VLD1.32         D8[0], [R5], R11
    VMLSL.S16       Q14, D11, D12

    VSHR.S32        Q1, Q1, #7
    VLD4.16         {D12, D13, D14, D15}, [R4]
    VADD.I32        Q2, Q1, Q0

    VLD1.32         D8[1], [R5], R11
    VSHR.S32        Q3, Q3, #7

    VSWP            Q15, Q14

    VLD1.32         D9[0], [R5], R11

    VSHR.S32        Q4, Q4, #7
    VST2.32         {Q14, Q15}, [R8]!
    ADD             R4, #24

    VUZP.16         D4, D5
    VSUB.I32        Q5, Q3, Q4

    VUZP.16         D10, D11
    VMULL.U16       Q15, D4, D12

    VMLAL.U16       Q15, D10, D13
    VMULL.U16       Q14, D4, D13

    VMLSL.U16       Q14, D10, D12
    VLD1.16         D0[0], [R4]!
    VSHR.U32        Q15, Q15, #16

    VSHR.S32        Q14, Q14, #16
    VLD1.32         D2[0], [R7], R11
    VMLAL.S16       Q15, D5, D12

    SUB             R5, R7, #124
    VMLAL.S16       Q15, D11, D13
    VLD1.32         D4[0], [R5]
    VMLAL.S16       Q14, D5, D13

    VMLSL.S16       Q14, D11, D12

    VSHR.S32        D2, D2, #7
    VST1.32         D30[0], [R8]!
    VSHR.S32        D4, D4, #7
    VSUB.I32        D2, D2, D4

    VMOV            D4, D2
    VST1.32         D28[0], [R8]!
    MOV             R6, R1

    ADD             R7, R1, #124
    VST1.32         D30[1], [R8]!
    ADD             R10, R3, #16

    SUB             R7, R7, #28
    VST1.32         D28[1], [R8]!
    MOV             R5, #-16

    MOV             R9, #-4
    VST1.32         D31[0], [R8]!
    MOV             R11, #16

    VST1.32         D29[0], [R8]!
    MOV             R12, #4

    VUZP.16         D4, D5
    MOV             R8, #6

    VLD1.16         D1[0], [R4], R8
    VMULL.U16       Q15, D4, D0


    VUZP.16         D2, D3
    VMULL.U16       Q14, D4, D1

    VMLAL.U16       Q15, D2, D1
    VLD2.32         {D10, D11}, [R6]
    VMLSL.U16       Q14, D2, D0

    ADD             R4, R3, #4
    MOV             R8, #-32
    VSHR.U32        Q15, Q15, #16
    VSHR.S32        Q14, Q14, #16
    VMLAL.S16       Q15, D5, D0
    VMLAL.S16       Q15, D3, D1
    VMLAL.S16       Q14, D5, D1
    VMLSL.S16       Q14, D3, D0

    VADD.I32        D14, D11, D28
    VLD2.32         {Q2, Q3}, [R7]
    VNEG.S32        D14, D14

    VREV64.32       Q2, Q2
    VSUB.I32        D12, D10, D30

    VREV64.32       Q3, Q3
    VADD.I32        D10, D10, D30

    VSWP            D4, D5
    VADD.I32        D10, D10, D14

    VSWP            D6, D7
    VSUB.I32        D11, D11, D28

    VADD.I32        D11, D11, D12
    VLD2.16         {D8, D9}, [R10], R5
    VSHR.S32        D10, D10, #1

    VREV64.16       D8, D8
    VSHR.S32        D11, D11, #1

    VUZP.32         D10, D11

    VST1.32         D10, [R6]!
    VLD2.32         {Q0, Q1}, [R6]

    VADD.I32        Q7, Q0, Q2
    VLD2.16         {D10, D11}, [R4], R11
    VSUB.I32        Q6, Q0, Q2

    VUZP.16         D12, D13
    VADD.I32        Q8, Q1, Q3

    VUZP.16         D16, D17
    VSUB.I32        Q9, Q1, Q3


    VMULL.U16       Q15, D12, D8
    VMLAL.U16       Q15, D16, D10
    VMULL.U16       Q14, D12, D10
    VMLSL.U16       Q14, D16, D8
    VSHR.S32        Q7, Q7, #1
    VSHR.U32        Q15, Q15, #16
    VSHR.S32        Q9, Q9, #1
    VSHR.S32        Q14, Q14, #16
    VMLAL.S16       Q15, D13, D8
    VMLAL.S16       Q15, D17, D10
    VMLAL.S16       Q14, D13, D10
    VMLSL.S16       Q14, D17, D8

    VSUB.I32        Q10, Q7, Q15
    VLD2.16         {D8, D9}, [R10]
    VADD.I32        Q13, Q7, Q15

    VREV64.32       Q13, Q13
    VSWP            D26, D27

    VADD.I32        Q11, Q9, Q14
    VREV64.16       D8, D8

    VSUB.I32        Q12, Q14, Q9

    VREV64.32       Q12, Q12

    VST2.32         {Q10, Q11}, [R6]!
    VSWP            D24, D25
    VSWP            Q12, Q13
    VST2.32         {Q12, Q13}, [R7], R8

    VLD2.32         {Q0, Q1}, [R6]
    VLD2.32         {Q2, Q3}, [R7]

    VREV64.32       Q2, Q2
    VREV64.32       Q3, Q3

    VSWP            D4, D5
    VSWP            D6, D7

    VSUB.I32        Q6, Q0, Q2
    VADD.I32        Q7, Q0, Q2
    VLD2.16         {D10, D11}, [R4], R11
    VADD.I32        Q8, Q1, Q3

    VUZP.16         D12, D13
    VSUB.I32        Q9, Q1, Q3


    VUZP.16         D16, D17
    VMULL.U16       Q15, D12, D8

    VMLAL.U16       Q15, D16, D10
    VMULL.U16       Q14, D12, D10
    VMLSL.U16       Q14, D16, D8
    ADD             R7, R7, #8
    VSHR.U32        Q15, Q15, #16
    VSHR.S32        Q7, Q7, #1
    VSHR.S32        Q14, Q14, #16
    VMLAL.S16       Q15, D13, D8
    VMLAL.S16       Q15, D17, D10
    VMLAL.S16       Q14, D13, D10
    VMLSL.S16       Q14, D17, D8

    VSHR.S32        Q9, Q9, #1
    VSUB.I32        Q10, Q7, Q15
    VSUB.I32        Q12, Q14, Q9

    VADD.I32        Q11, Q9, Q14
    VST1.32         D20[0], [R6]!
    VADD.I32        Q13, Q7, Q15


    VST1.32         D22[0], [R6]!
    VST1.32         D20[1], [R6]!
    VST1.32         D22[1], [R6]!
    VST1.32         D21[0], [R6]!
    VST1.32         D23[0], [R6]!

    VREV64.32       Q12, Q12

    VREV64.32       Q13, Q13
    VSWP            D24, D25
    VSWP            D26, D27


    VST1.32         D26[1], [R7]!
    VST1.32         D24[1], [R7]!
    VST1.32         D27[0], [R7]!
    VST1.32         D25[0], [R7]!
    VST1.32         D27[1], [R7]!
    VST1.32         D25[1], [R7]!

    SUB             R7, R7, #32
    VLD2.32         {D0, D1}, [R6]
    VLD2.32         {D2, D3}, [R7]

    VSUB.I32        D12, D0, D2
    VLD1.16         D8, [R10], R9
    VADD.I32        D14, D0, D2

    VADD.I32        D16, D1, D3
    VLD1.16         D10, [R4], R12
    VSUB.I32        D18, D1, D3

    VUZP.16         D12, D13
    MOV             R4, R0

    VUZP.16         D16, D17
    VMULL.U16       Q15, D12, D8
    VMLAL.U16       Q15, D16, D10
    VMULL.U16       Q14, D12, D10
    VMLSL.U16       Q14, D16, D8
    VSHR.S32        D18, D18, #1
    VSHR.U32        Q15, Q15, #16
    VSHR.S32        Q14, Q14, #16

    VMLAL.S16       Q15, D13, D8
    VMLAL.S16       Q15, D17, D10

    MOV             R10, R1

    VMLAL.S16       Q14, D13, D10
    VMLSL.S16       Q14, D17, D8
    VNEG.S32        Q15, Q15
    VSHR.S32        D14, D14, #1

    VADD.I32        Q13, Q7, Q15

    VADD.I32        Q11, Q9, Q14

    LDR             r0 , [sp , #104]
    VST1.32         D26[0], [R6]!
    MOV             r2, #1

    VST1.32         D22[0], [R6]!
    MOV             r3, #4
    BL              ixheaacd_radix4bfly

    MOV             r0, r4
    MOV             r1, r10
    LDR             r2 , [sp , #108]
    MOV             r3, #16
    BL              ixheaacd_postradixcompute4

    MOV             r0, r4
    MOV             r1, r10
    LDMIA           r0!, {r4, r5}
    STR             r4, [r1], #4
    STR             r5, [r1, #4]
    ADD             r2, r0, #64
    ADD             r3, r1, #116
    MOV             r6, #7

BACK3:

    LDMIA           r0!, {r4, r5}
    STR             r5, [r1], #8
    STR             r4, [r1], #8

    LDMIA           r2!, {r4, r5}
    STR             r5, [r3], #-8
    STR             r4, [r3], #-8

    SUBS            r6, r6, #1
    BNE             BACK3

    LDMIA           r0!, {r4, r5}
    STR             r5, [r1], #8
    STR             r4, [r1], #8

    VPOP            {D8 - D15}
    LDMFD           sp!, {r4-r12, r15}







