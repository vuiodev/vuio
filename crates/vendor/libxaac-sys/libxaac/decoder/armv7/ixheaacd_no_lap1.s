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
    .global ixheaacd_neg_shift_spec_armv7

ixheaacd_neg_shift_spec_armv7:
    STMFD           sp!, {R4-R12, R14}
    VPUSH           {D8 - D15}
    MOV             R5, #448
    SUB             R6, R5, #1
    MOV             R6, R6, LSL #2
    ADD             R6, R6, R0
    MOV             R8, #-16
    SUB             R6, R6, #12
    MOV             R7, R3, LSL #2
    VDUP.32         Q1, R2
    VLD1.32         {D0, D1}, [R6], R8
    VQNEG.S32       Q0, Q0


    VLD1.32         {D6, D7}, [R6], R8
    VQSHL.S32       Q15, Q0, Q1
    VMOV            Q13, Q15
    SUB             R5, R5, #8

    VQNEG.S32       Q3, Q3
    VREV64.32       Q13,Q13

LOOP_1:

   VST1.32         {D27[0]},[R1], R7
   VQSHL.S32       Q12, Q3, Q1
   VLD1.32         {D0, D1}, [R6], R8
   VST1.32         {D27[1]}, [R1], R7
   VST1.32         {D26[0]}, [R1], R7
   VQNEG.S32       Q0, Q0
   VST1.32         {D26[1]}, [R1], R7
   VMOV            Q10 , Q12
   SUBS            R5, R5, #8


   VREV64.32       Q10,Q10
   VQSHL.S32       Q15, Q0, Q1
   VST1.32         {D21[0]},[R1], R7
   VLD1.32         {D6, D7}, [R6],R8
   VST1.32         {D21[1]}, [R1],R7
   VMOV            Q13,Q15
   VST1.32         {D20[0]}, [R1],R7
   VST1.32         {D20[1]}, [R1],R7

   VREV64.32       Q13,Q13
   VQNEG.S32       Q3, Q3

   BGT             LOOP_1

   VST1.32         {D27[0]},[R1],R7
   VQSHL.S32       Q12, Q3, Q1
   VST1.32         {D27[1]}, [R1],R7
   VST1.32         {D26[0]}, [R1],R7
   VST1.32         {D26[1]}, [R1],R7
   VMOV             Q10, Q12

   VREV64.32       Q10,Q10
   VST1.32         {D21[0]}, [R1], R7
   VST1.32         {D21[1]}, [R1], R7
   VST1.32         {D20[0]}, [R1], R7
   VST1.32         {D20[1]}, [R1], R7
   VPOP            {D8 - D15}
   LDMFD           sp!, {R4-R12, R15}
.end

