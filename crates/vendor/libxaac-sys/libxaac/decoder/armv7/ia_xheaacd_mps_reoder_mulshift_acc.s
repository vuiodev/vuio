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
.global ixheaacd_mps_mulshift_acc

ixheaacd_mps_mulshift_acc:

    STMFD           sp!, {R4-R12, R14}
    VPUSH           {d8 - d15}
    LDR             R4, [SP, #104]      @Argument V_fix
    LDR             R5, [SP, #108]      @Argument 2*resolution
    ASR             R6, R5, #1

    MOV             R7, R4
    MOV             R11, #40
    MUL             R11, R11, R5
    ADD             R7, R7, R11
    LSL             R8, R5, #2
    SUB             R8, R7, R8
    MOV             R10, #9
    MUL             R9, R5, R10

COPYLOOP:
    SUB             R8, R8, #32
    VLD1.32         {Q0, Q1}, [R8]
    SUB             R7, R7, #32
    VST1.32         {Q0, Q1}, [R7]
    SUBS            R9, R9, #8
    BGT             COPYLOOP

LOOP:
    VMOV.I64        Q14, #0
    VMOV.I64        Q15, #0
    MOV             R7, R6
    MOV             R8, R0
    MOV             R9, R1

LOOP1:
    VLD1.32         {Q0, Q1}, [R8]!     @LOADING values from R0   Sr_fix
    VLD1.32         {Q2, Q3}, [R9]!     @LOADING values from R1   Si_fix
    VLD1.32         {Q4, Q5}, [R2]!     @LOADING values from R2   N.real_fix
    VLD1.32         {Q6, Q7}, [R3]!     @LOADING values from R3   N.imag_fix

    VMULL.S32       Q12, D0, D8
    VMULL.S32       Q10, D1, D9
    VMULL.S32       Q11, D3, D11
    VMULL.S32       Q13, D2, D10

    VMULL.S32       Q0, D4, D12
    VMULL.S32       Q4, D5, D13
    VMULL.S32       Q5, D7, D15
    VMULL.S32       Q1, D6, D14

    VSHR.S64        Q0, Q0, #31
    VSHR.S64        Q1, Q1, #31
    VSHR.S64        Q4, Q4, #31
    VSHR.S64        Q5, Q5, #31

    VSHR.S64        Q12, Q12, #31
    VSHR.S64        Q13, Q13, #31
    VSHR.S64        Q10, Q10, #31
    VSHR.S64        Q11, Q11, #31

    VSUB.I64        Q12, Q12, Q0
    VSUB.I64        Q13, Q13, Q1
    VSUB.I64        Q10, Q10, Q4
    VSUB.I64        Q11, Q11, Q5

    VADD.I64        Q12, Q12, Q13
    VADD.I64        Q10, Q10, Q11
    VADD.I64        Q12, Q12, Q10
    VADD.I64        D24, D24, D25
    VADD.I64        D28, D28, D24
    SUBS            R7, R7, #8
    BGT             LOOP1


    MOV             R7, R6
    MOV             R8, R0
    MOV             R9, R1

LOOP2:
    VLD1.32         {Q0, Q1}, [R8]!     @LOADING values from R0   Sr_fix
    VLD1.32         {Q2, Q3}, [R9]!     @LOADING values from R1   Si_fix
    VLD1.32         {Q4, Q5}, [R2]!     @LOADING values from R2   N.real_fix
    VLD1.32         {Q6, Q7}, [R3]!     @LOADING values from R3   N.imag_fix

    VMULL.S32       Q12, D0, D8
    VMULL.S32       Q10, D1, D9
    VMULL.S32       Q11, D3, D11
    VMULL.S32       Q13, D2, D10

    VMULL.S32       Q0, D4, D12
    VMULL.S32       Q4, D5, D13
    VMULL.S32       Q5, D7, D15
    VMULL.S32       Q1, D6, D14

    VSHR.S64        Q12, Q12, #31
    VSHR.S64        Q13, Q13, #31
    VSHR.S64        Q10, Q10, #31
    VSHR.S64        Q11, Q11, #31

    VSHR.S64        Q0, Q0, #31
    VSHR.S64        Q1, Q1, #31
    VSHR.S64        Q4, Q4, #31
    VSHR.S64        Q5, Q5, #31

    VSUB.I64        Q12, Q12, Q0
    VSUB.I64        Q13, Q13, Q1
    VSUB.I64        Q10, Q10, Q4
    VSUB.I64        Q11, Q11, Q5

    VADD.I64        Q12, Q12, Q13
    VADD.I64        Q10, Q10, Q11
    VADD.I64        Q12, Q12, Q10
    VADD.I64        D24, D24, D25
    VADD.I64        D29, D29, D24
    SUBS            R7, R7, #8
    BGT             LOOP2

    MOV             R7, R6
    MOV             R8, R0
    MOV             R9, R1


LOOP3:
    VLD1.32         {Q0, Q1}, [R8]!     @LOADING values from R0   Sr_fix
    VLD1.32         {Q2, Q3}, [R9]!     @LOADING values from R1   Si_fix
    VLD1.32         {Q4, Q5}, [R2]!     @LOADING values from R2   N.real_fix
    VLD1.32         {Q6, Q7}, [R3]!     @LOADING values from R3   N.imag_fix

    VMULL.S32       Q12, D0, D8
    VMULL.S32       Q10, D1, D9
    VMULL.S32       Q11, D3, D11
    VMULL.S32       Q13, D2, D10

    VMULL.S32       Q0, D4, D12
    VMULL.S32       Q4, D5, D13
    VMULL.S32       Q5, D7, D15
    VMULL.S32       Q1, D6, D14

    VSHR.S64        Q12, Q12, #31
    VSHR.S64        Q13, Q13, #31
    VSHR.S64        Q10, Q10, #31
    VSHR.S64        Q11, Q11, #31

    VSHR.S64        Q0, Q0, #31
    VSHR.S64        Q1, Q1, #31
    VSHR.S64        Q4, Q4, #31
    VSHR.S64        Q5, Q5, #31

    VSUB.I64        Q12, Q12, Q0
    VSUB.I64        Q13, Q13, Q1
    VSUB.I64        Q10, Q10, Q4
    VSUB.I64        Q11, Q11, Q5

    VADD.I64        Q12, Q12, Q13
    VADD.I64        Q10, Q10, Q11
    VADD.I64        Q12, Q12, Q10
    VADD.I64        D24, D24, D25
    VADD.I64        D30, D30, D24
    SUBS            R7, R7, #8
    BGT             LOOP3

    MOV             R7, R6
    MOV             R8, R0
    MOV             R9, R1


LOOP4:
    VLD1.32         {Q0, Q1}, [R8]!     @LOADING values from R0   Sr_fix
    VLD1.32         {Q2, Q3}, [R9]!     @LOADING values from R1   Si_fix
    VLD1.32         {Q4, Q5}, [R2]!     @LOADING values from R2   N.real_fix
    VLD1.32         {Q6, Q7}, [R3]!     @LOADING values from R3   N.imag_fix

    VMULL.S32       Q12, D0, D8
    VMULL.S32       Q10, D1, D9
    VMULL.S32       Q11, D3, D11
    VMULL.S32       Q13, D2, D10

    VMULL.S32       Q0, D4, D12
    VMULL.S32       Q4, D5, D13
    VMULL.S32       Q5, D7, D15
    VMULL.S32       Q1, D6, D14

    VSHR.S64        Q12, Q12, #31
    VSHR.S64        Q13, Q13, #31
    VSHR.S64        Q10, Q10, #31
    VSHR.S64        Q11, Q11, #31

    VSHR.S64        Q0, Q0, #31
    VSHR.S64        Q1, Q1, #31
    VSHR.S64        Q4, Q4, #31
    VSHR.S64        Q5, Q5, #31

    VSUB.I64        Q12, Q12, Q0
    VSUB.I64        Q13, Q13, Q1
    VSUB.I64        Q10, Q10, Q4
    VSUB.I64        Q11, Q11, Q5

    VADD.I64        Q12, Q12, Q13
    VADD.I64        Q10, Q10, Q11
    VADD.I64        Q12, Q12, Q10
    VADD.I64        D24, D24, D25
    VADD.I64        D31, D31, D24
    SUBS            R7, R7, #8
    BGT             LOOP4

    VQMOVN.S64      D0, Q14
    VQMOVN.S64      D1, Q15

    VST1.32         {Q0}, [R4]!         @Storing values to R4

    SUBS            R5, R5, #4
    BGT             LOOP

    VPOP            {d8 - d15}
    LDMFD           sp!, {R4-R12, R14}
    BX              LR

