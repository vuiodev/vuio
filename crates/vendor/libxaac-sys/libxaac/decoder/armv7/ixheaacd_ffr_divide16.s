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
        .global ixheaacd_fix_div_armv7

ixheaacd_fix_div_armv7:
    EOR             r12, r0, r1

    MOVS            r3, r1, ASR #1
    RSBMI           r3, r3, #0

    MOVS            r2, r0, ASR #1
    RSBMI           r2, r2, #0

    MOV             r0, #0
    BEQ             L2
    MOV             r1, #0xf
L1:
    MOV             r2, r2, LSL #1
    CMP             r2, r3
    MOV             r0, r0, LSL #1
    ADDCS           r0, r0, #1
    SUBCS           r2, r2, r3
    SUBS            r1, r1, #1
    BGT             L1
L2:
    CMP             r12, #0
    RSBLT           r0, r0, #0
    BX              lr
