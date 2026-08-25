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

    .global ixheaacd_imdct_using_fft_armv7
ixheaacd_imdct_using_fft_armv7:

    STMFD           sp!, {r4-r12, lr}
    vpush           {d8-d15}

@    LDR         r4, [sp, #0x68]
@    LDR         r5, [sp, #0x68+4]
@    LDR         r6, [sp, #0x68+8]
@    LDR         r7, [sp, #0x68+12]

    MOVW            r8, #11600
    ADD             r4, r0, r8
    MOVW            r8, #11856
    ADD             r5, r0, r8
    MOVW            r8, #11920
    ADD             r6, r0, r8
    MOVW            r8, #11936
    ADD             r7, r0, r8





COND_1: CMP         r1, #0x400
    BNE             COND_2

    MOV             r8, #4
    B               RADIX_4_FIRST_START


COND_2: CMP         r1, #0x200
    BNE             COND_3

    MOV             r8, #3
    MOV             r4, r5
    B               RADIX_8_FIRST_START

COND_3: CMP         r1, #0x100
    BNE             COND_4

    MOV             r8, #3
    MOV             r4, r5
    B               RADIX_4_FIRST_START

COND_4: CMP         r1, #0x80
    BNE             COND_5

    MOV             r8, #2
    MOV             r4, r6
    B               RADIX_8_FIRST_START

COND_5: CMP         r1, #0x40
    BNE             COND_6

    MOV             r8, #2
    MOV             r4, r6
    B               RADIX_4_FIRST_START
COND_6:

    MOV             r8, #1
    MOV             r4, r7






RADIX_8_FIRST_START:


    LSR             r9 , r1, #5
    LSL             r1, r1, #1

RADIX_8_FIRST_LOOP:

    MOV             r5 , r2
    MOV             r6 , r2
    MOV             r7 , r2
    MOV             r11 , r2

    LDRB            r12, [r4, #0]
    ADD             r5, r5, r12, LSL #3
    VLD2.32         {d0[0], d2[0]}, [r5] , r1
    ADD             r5, r5, r1
    VLD2.32         {d8[0], d10[0]}, [r5] , r1
    SUB             r5, r5, r1, LSL #1
    VLD2.32         {d4[0], d6[0]}, [r5] , r1
    ADD             r5, r5, r1
    VLD2.32         {d12[0], d14[0]}, [r5], r1
    SUB             r5, r5, r1, LSL #2

    LDRB            r12, [r4, #1]
    ADD             r6, r6, r12, LSL #3
    VLD2.32         {d0[1], d2[1]}, [r6] , r1
    ADD             r6, r6, r1
    VLD2.32         {d8[1], d10[1]}, [r6] , r1
    SUB             r6, r6, r1, LSL #1
    VLD2.32         {d4[1], d6[1]}, [r6] , r1
    ADD             r6, r6, r1
    VLD2.32         {d12[1], d14[1]}, [r6], r1
    SUB             r6, r6, r1, LSL #2


    LDRB            r12, [r4, #2]
    ADD             r7, r7, r12 , LSL #3
    VLD2.32         {d1[0], d3[0]}, [r7] , r1
    ADD             r7, r7, r1
    VLD2.32         {d9[0], d11[0]}, [r7] , r1
    SUB             r7, r7, r1, LSL #1

    LDRB            r12, [r4, #3]
    ADD             r11, r11, r12 , LSL #3
    VLD2.32         {d1[1], d3[1]}, [r11] , r1
    ADD             r11, r11, r1
    VLD2.32         {d9[1], d11[1]}, [r11] , r1
    SUB             r11, r11, r1, LSL #1



    VADD.I32        q8, q0, q4
    VLD2.32         {d5[0], d7[0]}, [r7] , r1
    ADD             r7, r7, r1

    VSUB.I32        q9, q0, q4
    VLD2.32         {d13[0], d15[0]}, [r7], r1
    SUB             r7, r7, r1, LSL #2




    VADD.I32        q0, q1, q5
    VLD2.32         {d5[1], d7[1]}, [r11] , r1
    ADD             r11, r11, r1

    VSUB.I32        q4, q1, q5
    VLD2.32         {d13[1], d15[1]}, [r11], r1
    SUB             r11, r11, r1, LSL #2



    ADD             r4, r4, #4

    ADD             r5, r5, r1, LSR #1
    ADD             r6, r6, r1, LSR #1
    ADD             r7, r7, r1, LSR #1
    ADD             r11, r11, r1, LSR #1


    VADD.I32        q1, q2, q6
    VLD2.32         {d28[0], d30[0]}, [r5] , r1


    VSUB.I32        q5, q2, q6
    VLD2.32         {d20[0], d22[0]}, [r5] , r1


    VADD.I32        q2, q3, q7
    VLD2.32         {d24[0], d26[0]}, [r5] , r1


    VSUB.I32        q6, q3, q7
    VLD2.32         {d28[1], d30[1]}, [r6] , r1

    VADD.S32        q3, q9, q6
    VLD2.32         {d20[1], d22[1]}, [r6] , r1

    VSUB.S32        q7, q9, q6
    VLD2.32         {d24[1], d26[1]}, [r6] , r1

    VSUB.S32        q6, q4, q5
    VLD2.32         {d29[0], d31[0]}, [r7] , r1

    VADD.S32        q9, q4, q5
    VLD2.32         {d21[0], d23[0]}, [r7] , r1

    VADD.S32        q4, q8, q1
    VLD2.32         {d25[0], d27[0]}, [r7] , r1

    VSUB.S32        q5, q8, q1
    VLD2.32         {d29[1], d31[1]}, [r11] , r1

    VADD.S32        q8, q0, q2
    VLD2.32         {d21[1], d23[1]}, [r11] , r1

    VSUB.S32        q0, q0, q2
    VLD2.32         {d25[1], d27[1]}, [r11] , r1


    VPUSH           {q3}
    VPUSH           {q7}








    VLD2.32         {d2[0], d4[0]}, [r5], r1

    VADD.I32        q7, q14, q12

    VLD2.32         {d2[1], d4[1]}, [r6] , r1

    VSUB.I32        q3, q14, q12

    VLD2.32         {d3[0], d5[0]}, [r7] , r1

    VADD.I32        q14, q15, q13

    VLD2.32         {d3[1], d5[1]}, [r11] , r1

    VSUB.I32        q12, q15, q13









    VADD.I32        q15, q10, q1
    VSUB.I32        q13, q10, q1
    VADD.I32        q10, q11, q2
    VSUB.I32        q1, q11, q2



    VADD.S32        q11, q7, q15
    VSUB.S32        q2, q7, q15
    VADD.S32        q7, q14, q10
    VSUB.S32        q15, q14, q10

    VADD.S32        q14, q3, q12
    VSUB.S32        q10, q3, q12
    VADD.S32        q3, q13, q1
    VSUB.S32        q12, q13, q1

    VADD.S32        q1 , q14, q12
    VSUB.S32        q13, q14, q12
    VSUB.S32        q12, q3, q10

    VUZP.16         d2, d3
    VADD.S32        q14, q3, q10

    VUZP.16         d26, d27
    VADD.S32        q3, q4, q11

    VUZP.16         d24, d25
    VSUB.S32        q10, q4, q11

    VUZP.16         d28, d29
    VADD.S32        q4, q8, q7

    MOVW            r14, #0x5a82

    VSUB.S32        q11, q8, q7

    VADD.S32        q8, q5, q15
    VSUB.S32        q7, q5, q15
    VSUB.S32        q5, q0, q2
    VADD.S32        q15, q0, q2

    VPOP            {q0}
    VPOP            {q2}
    VPUSH           {q3-q4}
    VPUSH           {q10}

















    VDUP.16         d20, r14


    VMULL.u16       q4, d26, d20
    VMULL.u16       q3, d28, d20

    VPUSH           {q7-q8}
    VPUSH           {q5}

    VSHR.S32        q4, q4, #15
    VSHR.S32        q3, q3, #15

    VQDMLAL.S16     q4, d27, d20
    VQDMLAL.S16     q3, d29, d20


    VPUSH           {q11}

    VMULL.u16       q13, d24, d20
    VMULL.u16       q14, d2, d20

    VADD.S32        q5, q2, q4
    VSUB.S32        q7, q2, q4

    VADD.S32        q8, q6, q3
    VSUB.S32        q6, q6, q3






    VSHR.S32        q13, q13, #15
    VSHR.S32        q14, q14, #15

    VQDMLAL.S16     q13, d25, d20
    VQDMLAL.S16     q14, d3, d20

    VPOP            {q1}
    VPOP            {q10}

    VADD.S32        q2, q0, q13
    VSUB.S32        q4, q0, q13

    VADD.S32        q11, q9, q14
    VSUB.S32        q3, q9, q14




    VPOP            {q14}
    VPOP            {q9}
    VPOP            {q0}
    VPOP            {q12, q13}





























    VTRN.32         q12, q5
    VSHL.S32        q12, q12, #3
    VTRN.32         q9, q2
    VSHL.S32        q5, q5, #3

    VSHL.S32        q9, q9, #3
    VTRN.32         q0, q7
    VSHL.S32        q2, q2, #3

    VSHL.S32        q0, q0, #3
    VTRN.32         q14, q4
    VSHL.S32        q7, q7, #3

    VSHL.S32        q14, q14, #3
    VTRN.32         q13, q6
    VSHL.S32        q4, q4, #3

    VSHL.S32        q13, q13, #3
    VTRN.32         q10, q3
    VSHL.S32        q6, q6, #3

    VSHL.S32        q10, q10, #3
    VTRN.32         q1, q8
    VSHL.S32        q3, q3, #3

    VSHL.S32        q1, q1, #3
    VTRN.32         q15, q11
    VSHL.S32        q8, q8, #3

    VSHL.S32        q15, q15, #3
    VSWP            d18, d25

    VSHL.S32        q11, q11, #3
    VSWP            d4, d11

    VSWP            d1, d28
    VSWP            d15, d8

    VSWP            d20, d27
    VSWP            d6, d13

    VSWP            d30, d3
    VSWP            d22, d17

    VST2.32         {q12, q13}, [r3]!
    VST2.32         {q0, q1}, [r3]!

    VST2.32         {q5, q6}, [r3]!
    VST2.32         {q7, q8}, [r3]!

    VMOV            q5, q11

    VST2.32         {q9, q10}, [r3]!
    VST2.32         {q14, q15}, [r3]!

    VST2.32         {q2, q3}, [r3]!
    VST2.32         {q4, q5}, [r3]!


    SUBS            r9, r9, #1
    BNE             RADIX_8_FIRST_LOOP

    LSR             r1, r1, #1
    SUB             r3, r1, LSL #3

    MOV             r5, #8
    MOV             r4, #32
    LSR             r6, r1, #5

    B               RADIX_4_FIRST_ENDS

RADIX_8_FIRST_ENDS:


RADIX_4_FIRST_START:


    LSR             r9 , r1, #4
    LSL             r1, r1, #1

RADIX_4_LOOP:

    MOV             r5 , r2
    MOV             r6 , r2
    MOV             r7 , r2
    MOV             r11 , r2















    LDRB            r12, [r4, #0]
    ADD             r5, r5, r12, LSL #3

    VLD2.32         {d0[0], d2[0]}, [r5] , r1
    ADD             r5, r5, r1
    VLD2.32         {d8[0], d10[0]}, [r5] , r1
    SUB             r5, r5, r1, LSL #1
    VLD2.32         {d4[0], d6[0]}, [r5] , r1
    ADD             r5, r5, r1
    VLD2.32         {d12[0], d14[0]}, [r5], r1

    LDRB            r12, [r4, #1]
    ADD             r6, r6, r12, LSL #3

    VLD2.32         {d0[1], d2[1]}, [r6] , r1
    ADD             r6, r6, r1
    VLD2.32         {d8[1], d10[1]}, [r6] , r1
    SUB             r6, r6, r1, LSL #1
    VLD2.32         {d4[1], d6[1]}, [r6] , r1
    ADD             r6, r6, r1
    VLD2.32         {d12[1], d14[1]}, [r6], r1


    LDRB            r12, [r4, #2]
    ADD             r7, r7, r12, LSL #3

    VLD2.32         {d1[0], d3[0]}, [r7] , r1
    ADD             r7, r7, r1
    VLD2.32         {d9[0], d11[0]}, [r7] , r1

    LDRB            r12, [r4, #3]
    ADD             r11, r11, r12 , LSL #3

    VLD2.32         {d1[1], d3[1]}, [r11] , r1
    ADD             r11, r11, r1
    VLD2.32         {d9[1], d11[1]}, [r11] , r1


    SUB             r7, r7, r1, LSL #1
    VADD.S32        q8, q0, q4
    VLD2.32         {d5[0], d7[0]}, [r7] , r1
    ADD             r7, r7, r1
    VADD.S32        q9, q1, q5
    VLD2.32         {d13[0], d15[0]}, [r7], r1



    SUB             r11, r11, r1, LSL #1
    VSUB.S32        q10, q0, q4
    VLD2.32         {d5[1], d7[1]}, [r11] , r1
    ADD             r11, r11, r1
    VSUB.S32        q11, q1, q5
    VLD2.32         {d13[1], d15[1]}, [r11], r1


    ADD             r4, r4, #4

    VADD.S32        q12, q2, q6
    VADD.S32        q13, q3, q7
    VSUB.S32        q14, q2, q6
    VSUB.S32        q15, q3, q7

    VADD.S32        q0, q8, q12
    VADD.S32        q1, q9, q13
    VSUB.S32        q2, q8, q12
    VSUB.S32        q3, q9, q13

    VADD.S32        q4, q10, q15
    VSUB.S32        q5, q11, q14
    VADD.S32        q7, q11, q14
    VSUB.S32        q6, q10, q15




    VTRN.32         q0, q4

    VSHL.S32        q0, q0, #2
    VTRN.32         q2, q6
    VSHL.S32        q4, q4, #2

    VSHL.S32        q2, q2, #2
    VTRN.32         q1, q5
    VSHL.S32        q6, q6, #2

    VSHL.S32        q1, q1, #2
    VTRN.32         q3, q7
    VSHL.S32        q5, q5, #2

    VSHL.S32        q3, q3, #2
    VSWP            d4, d1

    VSHL.S32        q7, q7, #2
    VSWP            d12, d9



    VSWP            d6, d3
    VSWP            d14, d11


    VST2.32         {q0, q1}, [r3]!
    VST2.32         {q4, q5}, [r3]!

    VST2.32         {q2, q3}, [r3]!
    VST2.32         {q6, q7}, [r3]!



    SUBS            r9, r9, #1
    BNE             RADIX_4_LOOP

    LSR             r1, r1, #1
    SUB             r3, r1, LSL #3
    MOV             r5, #4
    MOV             r4, #64
    LSR             r6, r1, #4


RADIX_4_FIRST_ENDS:

    PUSH            {r3}
    LSR             r5, r5, #2
    MOVW            r14, #8528
    ADD             r0, r0, r14

OUTER_LOOP_R4:


    LDR             r14, [sp]

    MOV             r7, r5
    MOV             r2, #0
    MOV             r9, r0
    LSL             r12 , r5, #5
MIDDLE_LOOP_R4:


    VLD2.16         {d0[0], d1[0]}, [r9], r2
    VLD2.16         {d2[0], d3[0]}, [r9], r2
    ADD             r11, r2, r4, LSL #2
    VLD2.16         {d4[0], d5[0]}, [r9]
    ADD             r10, r0, r11


    VLD2.16         {d0[1], d1[1]}, [r10], r11
    VLD2.16         {d2[1], d3[1]}, [r10], r11
    ADD             r2, r11, r4, LSL #2
    VLD2.16         {d4[1], d5[1]}, [r10]
    ADD             r9, r0, r2


    VLD2.16         {d0[2], d1[2]}, [r9], r2
    VLD2.16         {d2[2], d3[2]}, [r9], r2
    ADD             r11, r2, r4, LSL #2
    VLD2.16         {d4[2], d5[2]}, [r9]
    ADD             r10, r0, r11



    VLD2.16         {d0[3], d1[3]}, [r10], r11
    VLD2.16         {d2[3], d3[3]}, [r10], r11
    ADD             r2, r11, r4, LSL #2
    VLD2.16         {d4[3], d5[3]}, [r10]
    ADD             r9, r0, r2

    MOV             r10, r6



INNER_LOOP_R4:

    VLD2.32         {q3, q4}, [r14], r12

    VSHR.S32        q3, q3, #1
    VLD4.16         {q5, q6}, [r14], r12
    VSHR.S32        q4, q4, #1

    VSHR.U16        d10, d10, #1
    VLD4.16         {q7, q8}, [r14], r12
    VSHR.U16        d12, d12, #1

    VMULL.S16       q11, d10, d0
    VMLSL.S16       q11, d12, d1
    VLD4.16         {q9, q10}, [r14], r12
    VMULL.S16       q12, d10, d1
    VMLAL.S16       q12, d12, d0

    VSHR.U16        d14, d14, #1
    VSHR.U16        d16, d16, #1

    SUB             r14, r14, r12, LSL #2

    VSHR.U16        d18, d18, #1
    VSHR.U16        d20, d20, #1

    VMULL.S16       q13, d14, d2
    VMLSL.S16       q13, d16, d3

    VSHR.S32        q11, q11, #15

    VMULL.S16       q14, d14, d3
    VMLAL.S16       q14, d16, d2

    VMULL.S16       q15, d18, d4
    VMLSL.S16       q15, d20, d5

    VMLAL.S16       q11, d11, d0
    VMLSL.S16       q11, d13, d1

    VSHR.S32        q12, q12, #15
    VSHR.S32        q13, q13, #15
    VSHR.S32        q14, q14, #15
    VSHR.S32        q15, q15, #15


    VMLAL.S16       q12, d11, d1
    VMLAL.S16       q12, d13, d0


    VMULL.S16       q5, d18, d5
    VMLAL.S16       q5, d20, d4


    VMLAL.S16       q13, d15, d2
    VMLSL.S16       q13, d17, d3

    VMLAL.S16       q14, d15, d3
    VMLAL.S16       q14, d17, d2


    VMLAL.S16       q15, d19, d4
    VMLSL.S16       q15, d21, d5

    VSHR.S32        q5, q5, #15

    VMLAL.S16       q5, d19, d5
    VMLAL.S16       q5, d21, d4



    CMP             r7, r5
    BNE             BYPASS_IF

    ADD             r14, r14, r12

    LDR             r3, [r14], r12
    ASR             r3, r3, #1
    VMOV.32         d22[0], r3

    LDR             r3, [r14], r12
    ASR             r3, r3, #1
    VMOV.32         d26[0], r3

    LDR             r3, [r14]
    ASR             r3, r3, #1
    VMOV.32         d30[0], r3

    SUB             r14, r14, r12, LSL #1
    ADD             r14, r14, #4

    LDR             r3, [r14], r12
    ASR             r3, r3, #1
    VMOV.32         d24[0], r3

    LDR             r3, [r14], r12
    ASR             r3, r3, #1
    VMOV.32         d28[0], r3

    LDR             r3, [r14], r12
    ASR             r3, r3, #1
    VMOV.32         d10[0], r3

    SUB             r14, r14, #4

    SUB             r14, r14, r12, LSL #2

BYPASS_IF:

    VADD.S32        q6, q3, q13
    VADD.S32        q7, q4, q14
    VSUB.S32        q3, q3, q13
    VSUB.S32        q4, q4, q14
    VADD.S32        q8, q11, q15
    VADD.S32        q9, q12, q5

    VSUB.S32        q15, q11, q15
    VSUB.S32        q14, q12, q5

    VADD.S32        q10, q6, q8
    VADD.S32        q11, q7, q9
    VADD.S32        q12, q3, q14
    VSUB.S32        q13, q4, q15

    VSUB.S32        q6, q6, q8
    VST2.32         {q10, q11}, [r14], r12
    VSUB.S32        q7, q7, q9

    VSUB.S32        q8, q3, q14
    VST2.32         {q12, q13}, [r14], r12
    VADD.S32        q9, q4, q15


    VST2.32         {q6, q7}, [r14], r12
    VST2.32         {q8, q9}, [r14], r12




    SUBS            r10, r10, #1
    BNE             INNER_LOOP_R4

    SUB             r14, r14, r1, LSL #3
    ADD             r14, r14, #32

    SUBS            r7, r7, #1
    BNE             MIDDLE_LOOP_R4




    LSR             r4, r4, #2
    LSL             r5, r5, #2
    LSR             r6, r6, #2
    SUBS            r8, r8, #1
    BNE             OUTER_LOOP_R4
END_LOOPS:
    POP             {r3}
    vpop            {d8-d15}
    LDMFD           sp!, {r4-r12, pc}

