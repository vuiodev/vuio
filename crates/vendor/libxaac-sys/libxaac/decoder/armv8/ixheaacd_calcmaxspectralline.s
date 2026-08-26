///******************************************************************************
// *
// * Copyright (C) 2018 The Android Open Source Project
// *
// * Licensed under the Apache License, Version 2.0 (the "License");
// * you may not use this file except in compliance with the License.
// * You may obtain a copy of the License at:
// *
// * http://www.apache.org/licenses/LICENSE-2.0
// *
// * Unless required by applicable law or agreed to in writing, software
// * distributed under the License is distributed on an "AS IS" BASIS,
// * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// * See the License for the specific language governing permissions and
// * limitations under the License.
// *
// *****************************************************************************
// * Originally developed and contributed by Ittiam Systems Pvt. Ltd, Bangalore
//*/


.text
.global ixheaacd_calc_max_spectral_line_armv8
ixheaacd_calc_max_spectral_line_armv8:

    LSR             W4, W1, #3
    LSL             W6, W4, #3
    MOV             w11, #0x00000000
    MOV             V3.S[0], w11
    MOV             V3.S[1], w11
    MOV             V3.S[2], w11
    MOV             V3.S[3], w11

LOOP_1:
    LD1             {V0.4S}, [X0], #16
    LD1             {V1.4S}, [X0], #16

    ABS             V0.4S, V0.4S
    ABS             V1.4S, V1.4S

    SUBS            W4, W4, #1

    ORR             V3.16B, V0.16B, V3.16B
    ORR             V3.16B, V1.16B, V3.16B

    BGT             LOOP_1

    SUBS            W7, W1, W6

    MOV             W4, V3.S[0]
    MOV             W1, V3.S[1]
    MOV             W2, V3.S[2]
    ORR             W4, W4, W1
    MOV             W3, V3.S[3]
    ORR             W4, W4, W2
    ORR             W4, W4, W3
    BEQ             END_FUNC
LOOP_2:

    LDR             W2, [X0], #4

    CMP             W2, #0

    CNEG            W2, W2, LE
    ORR             W4, W4, W2
    SUBS            W7, W7, #1
    BGT             LOOP_2

END_FUNC:

    MOV             W0, W4
    CMP             W0, #0

    CNEG            W0, W0, LE
    CLZ             W0, W0
    SUB             W0, W0, #1

    RET




