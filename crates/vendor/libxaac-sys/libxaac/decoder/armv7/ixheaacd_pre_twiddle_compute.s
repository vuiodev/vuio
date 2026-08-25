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
.global ixheaacd_pretwiddle_compute_armv7

ixheaacd_pretwiddle_compute_armv7:

    STMFD           sp!, {R4-R12}
    VPUSH           {d8 - d15}

    MOVW            R8, #7500
    ADD             R3, R3, R8
    LDR             R4, [sp, #100]
    LDR             R5, [sp, #104]

    LSL             R7, R4, #4
    ADD             R7, R2, R7
    SUB             R7, R7, #4

    MVN             R5, R5
    ADD             R5, R5, #1

ARM_PROLOGUE:
    LDR             R8, [R3], #4
    LDR             R9, [R0], #4

    SMULWB          R12, R9, R8
    LDR             R10, [R1], #-4
    SMULWT          R11, R9, R8
    SMLAWT          R9, R10, R8, R12
    SMULWB          R6, R10, R8

    MVN             R9, R9
    ADD             R9, R9, #1

    SUB             R11, R11, R6

    CMP             R5, #0
    BGT             NEXT
    MVN             R8, R5
    ADD             R8, R8, #1
    ASR             R11, R11, R8
    ASR             R9, R9, R8
    B               NEXT1

NEXT:
    LSL             R11, R11, R5
    LSL             R9, R9, R5



NEXT1:
    STR             R9, [R2], #4
    STR             R11, [R2], #4

    CMP             R4, #0x100
    BNE             NXT
    MOV             R6, #4
    B               NXT1
NXT:
    MOV             R6, #32
    ADD             R3, R3, #28

NXT1:
    SUB             R4, R4, #1
    ASR             R4, R4, #2
    SUB             R7, R7, #28












NEON_PROLOGUE:

    MOV             R8, #-32
    VDUP.32         Q7, R5
    SUB             R1, R1, #28

    VLD2.16         {D8[0], D9[0]}, [R3], R6
    VLD2.16         {D8[1], D9[1]}, [R3], R6
    VLD2.16         {D8[2], D9[2]}, [R3], R6
    VLD2.16         {D8[3], D9[3]}, [R3], R6

    VREV64.16       Q5, Q4

    VLD4.16         {D0, D1, D2, D3}, [R0]!
    VLD4.16         {D4, D5, D6, D7}, [R1], R8

    VREV64.16       Q0, Q0
    VREV64.16       Q2, Q2






    VMULL.U16       Q15, D2, D9
    VMULL.U16       Q14, D4, D9
    VMULL.U16       Q13, D2, D8
    VMULL.U16       Q12, D4, D8

    VSHR.U32        Q15, Q15, #16
    VSHR.U32        Q14, Q14, #16
    VSHR.U32        Q13, Q13, #16
    VSHR.U32        Q12, Q12, #16

    VMLAL.S16       Q15, D3, D9
    VMLAL.S16       Q14, D5, D9
    VMLAL.S16       Q13, D3, D8
    VMLAL.S16       Q12, D5, D8

    VADD.I32        Q14, Q13, Q14
    VNEG.S32        Q14, Q14
    VSUB.I32        Q15, Q15, Q12

    VMULL.U16       Q11, D0, D11
    VMULL.U16       Q10, D6, D11
    VMULL.U16       Q9, D0, D10
    VMULL.U16       Q8, D6, D10

    VSHR.U32        Q11, Q11, #16
    VSHR.U32        Q10, Q10, #16
    VSHR.U32        Q9, Q9, #16
    VSHR.U32        Q8, Q8, #16

    VMLAL.S16       Q11, D1, D11
    VLD2.16         {D8[0], D9[0]}, [R3], R6

    VMLAL.S16       Q10, D7, D11
    VLD2.16         {D8[1], D9[1]}, [R3], R6

    VMLAL.S16       Q9, D1, D10
    VLD2.16         {D8[2], D9[2]}, [R3], R6

    VMLAL.S16       Q8, D7, D10
    VLD2.16         {D8[3], D9[3]}, [R3], R6

    VADD.I32        Q10, Q10, Q9

    VNEG.S32        Q10, Q10
    VREV64.16       Q5, Q4

    VSUB.I32        Q11, Q8, Q11
    VLD4.16         {D0, D1, D2, D3}, [R0]!



    VSHL.S32        Q10, Q10, Q7
    VLD4.16         {D4, D5, D6, D7}, [R1], R8

    VREV64.16       Q0, Q0
    VSHL.S32        Q11, Q11, Q7

    VREV64.16       Q2, Q2
    VSHL.S32        Q9, Q15, Q7
    VSHL.S32        Q8, Q14, Q7



    SUB             R4, R4, #2

CORE_LOOP_PTC:
    VMULL.U16       Q15, D2, D9
    VST2.32         {Q8, Q9}, [R2]!
    VMULL.U16       Q14, D4, D9

    VMULL.U16       Q13, D2, D8
    VST2.32         {Q10, Q11}, [R7], R8
    VMULL.U16       Q12, D4, D8

    VSHR.U32        Q15, Q15, #16
    VSHR.U32        Q14, Q14, #16
    VSHR.U32        Q13, Q13, #16
    VSHR.U32        Q12, Q12, #16

    VMLAL.S16       Q15, D3, D9
    VMLAL.S16       Q14, D5, D9
    VMLAL.S16       Q13, D3, D8
    VMLAL.S16       Q12, D5, D8

    VADD.I32        Q14, Q13, Q14
    VNEG.S32        Q14, Q14
    VSUB.I32        Q15, Q15, Q12

    VMULL.U16       Q11, D0, D11
    VLD2.16         {D8[0], D9[0]}, [R3], R6
    VMULL.U16       Q10, D6, D11

    VMULL.U16       Q9, D0, D10
    VLD2.16         {D8[1], D9[1]}, [R3], R6
    VMULL.U16       Q8, D6, D10

    VSHR.U32        Q11, Q11, #16
    VLD2.16         {D8[2], D9[2]}, [R3], R6
    VSHR.U32        Q10, Q10, #16

    VSHR.U32        Q9, Q9, #16
    VLD2.16         {D8[3], D9[3]}, [R3], R6
    VSHR.U32        Q8, Q8, #16

    VMLAL.S16       Q11, D1, D11
    VMLAL.S16       Q10, D7, D11
    VMLAL.S16       Q9, D1, D10
    VMLAL.S16       Q8, D7, D10

    VLD4.16         {D0, D1, D2, D3}, [R0]!
    VADD.I32        Q10, Q10, Q9

    VNEG.S32        Q10, Q10
    VREV64.16       Q5, Q4

    VSUB.I32        Q11, Q8, Q11
    VLD4.16         {D4, D5, D6, D7}, [R1], R8
    VSHL.S32        Q10, Q10, Q7
    VSHL.S32        Q11, Q11, Q7

    VREV64.16       Q0, Q0
    VSHL.S32        Q9, Q15, Q7

    VREV64.16       Q2, Q2
    VSHL.S32        Q8, Q14, Q7

    SUBS            R4, R4, #1
    BNE             CORE_LOOP_PTC

NEON_EPILOGUE:
    VMULL.U16       Q15, D2, D9
    VST2.32         {Q8, Q9}, [R2]!
    VMULL.U16       Q14, D4, D9

    VMULL.U16       Q13, D2, D8
    VST2.32         {Q10, Q11}, [R7], R8
    VMULL.U16       Q12, D4, D8

    VSHR.U32        Q15, Q15, #16
    VSHR.U32        Q14, Q14, #16
    VSHR.U32        Q13, Q13, #16
    VSHR.U32        Q12, Q12, #16

    VMLAL.S16       Q15, D3, D9
    VMLAL.S16       Q14, D5, D9
    VMLAL.S16       Q13, D3, D8
    VMLAL.S16       Q12, D5, D8

    VADD.I32        Q14, Q13, Q14
    VNEG.S32        Q14, Q14
    VSUB.I32        Q15, Q15, Q12

    VMULL.U16       Q11, D0, D11
    VMULL.U16       Q10, D6, D11
    VMULL.U16       Q9, D0, D10
    VMULL.U16       Q8, D6, D10

    VSHR.U32        Q11, Q11, #16
    VSHR.U32        Q10, Q10, #16
    VSHR.U32        Q9, Q9, #16
    VSHR.U32        Q8, Q8, #16

    VMLAL.S16       Q11, D1, D11
    VMLAL.S16       Q10, D7, D11
    VMLAL.S16       Q9, D1, D10
    VMLAL.S16       Q8, D7, D10

    VADD.I32        Q10, Q10, Q9
    VNEG.S32        Q10, Q10
    VSUB.I32        Q11, Q8, Q11


    VSHL.S32        Q10, Q10, Q7
    VSHL.S32        Q11, Q11, Q7
    VSHL.S32        Q9, Q15, Q7
    VSHL.S32        Q8, Q14, Q7

    VST2.32         {Q8, Q9}, [R2]!
    VST2.32         {Q10, Q11}, [R7], R8


RESIDUE_NEON:
    MOV             R10, #-16
    VMOV.S32        D3, #0x00000000
    VMOV.S32        D4, #0x00000000

    VLD2.32         {D0, D2}, [R0]!
    VLD2.32         {D1[0], D3[0]}, [R0]!
    VLD1.32         D1[1], [R0]

    VUZP.16         D0, D1
    VUZP.16         D2, D3

    ADD             R1, R1, #4

    VLD1.32         D6[0], [R1]!
    VLD2.32         {D4[1], D6[1]}, [R1]!
    VLD2.32         {D5, D7}, [R1]!

    VUZP.16         D4, D5
    VUZP.16         D6, D7

    VREV64.16       Q0, Q0
    VREV64.16       Q2, Q2

    VLD2.16         {D8[0], D9[0]}, [R3], R6
    VLD2.16         {D8[1], D9[1]}, [R3], R6
    VLD2.16         {D8[2], D9[2]}, [R3], R6
    VLD2.16         {D8[3], D9[3]}, [R3], R6

    VREV64.16       Q5, Q4


    VMULL.U16       Q15, D2, D9
    VMULL.U16       Q14, D4, D9
    VMULL.U16       Q13, D2, D8
    VMULL.U16       Q12, D4, D8

    VSHR.U32        Q15, Q15, #16
    VSHR.U32        Q14, Q14, #16
    VSHR.U32        Q13, Q13, #16
    VSHR.U32        Q12, Q12, #16

    VMLAL.S16       Q15, D3, D9
    VMLAL.S16       Q14, D5, D9
    VMLAL.S16       Q13, D3, D8
    VMLAL.S16       Q12, D5, D8

    VADD.I32        Q14, Q13, Q14
    VNEG.S32        Q14, Q14
    VSUB.I32        Q15, Q15, Q12

    VMULL.U16       Q11, D0, D11
    VMULL.U16       Q10, D6, D11
    VMULL.U16       Q9, D0, D10
    VMULL.U16       Q8, D6, D10

    VSHR.U32        Q11, Q11, #16
    VSHR.U32        Q10, Q10, #16
    VSHR.U32        Q9, Q9, #16
    VSHR.U32        Q8, Q8, #16

    VMLAL.S16       Q11, D1, D11
    VMLAL.S16       Q10, D7, D11
    VMLAL.S16       Q9, D1, D10
    VMLAL.S16       Q8, D7, D10

    VADD.I32        Q10, Q10, Q9
    VNEG.S32        Q10, Q10
    VSUB.I32        Q11, Q8, Q11


    VSHL.S32        Q10, Q10, Q7
    VSHL.S32        Q11, Q11, Q7
    VSHL.S32        Q9, Q15, Q7
    VSHL.S32        Q8, Q14, Q7

    VST2.32         {Q10, Q11}, [R7]
    VST2.32         {D16, D18}, [R2]!
    VST2.32         {D17[0], D19[0]}, [R2]!

    VPOP            {d8 - d15}
    LDMFD           sp!, {R4-R12}
    BX              LR

