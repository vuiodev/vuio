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
        .global ixheaacd_mps_synt_pre_twiddle_armv7


ixheaacd_mps_synt_pre_twiddle_armv7:

    STMFD           sp!, {r4-r12, r14}
    VPUSH           {D8-D15}
    lsl             R3, R3, #1
LOOP1:
    VLD1.32         {D0}, [R2]!
    VLD1.32         {D1}, [R1]!
    VLD2.32         {D2, D3}, [R0]
    VNEG.S32        D12, D2

    VMULL.S32       Q2, D0, D12
    VMULL.S32       Q3, D0, D3
    VMULL.S32       Q4, D1, D2
    VMULL.S32       Q5, D1, D3

    VSHRN.I64       D4, Q2, #31
    VSHRN.I64       D6, Q3, #31
    VSHRN.I64       D8, Q4, #31
    VSHRN.I64       D10, Q5, #31

    VQADD.S32       D0, D8, D6
    VQADD.S32       D1, D4, D10

    SUBS            R3, R3, #4
    VST2.32         {D0, D1} , [R0]!
    BGT             LOOP1

    VPOP            {D8-D15}
    LDMFD           sp!, {r4-r12, r15}




