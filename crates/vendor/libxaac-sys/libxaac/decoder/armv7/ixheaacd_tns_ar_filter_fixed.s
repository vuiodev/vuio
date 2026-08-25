
@/******************************************************************************
@ *
@ * Copyright (C) 2018 The Android Open Source Project
@ *
@ * Licensed under the Apache License, Version 2.0 (the "License")@
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

.text
.p2align 2
        .global ixheaacd_tns_ar_filter_fixed_armv7

ixheaacd_tns_ar_filter_fixed_armv7:
    STMFD           r13! , {r4 - r12, r14}
    vpush           {d8-d15}
    SUB             sp, sp, #128        @ state[MaximumOrder] + one more
    LDR             r4, [sp, #232]      @order
    LDR             r6, [sp, #236]      @shift_value
    STR             r1, [sp]
    ADD             r12, sp, #4         @ r12 = state
    ANDS            r5, r4, #3
    BEQ             FILTER_LOOP
    MOV             r8, #0
    ADD             r14, r3, r4, LSL #2
    RSBS            r7, r5, #3
    BEQ             ORDER_LOOPEND
ORDER_LOOP:
    STR             r8, [r14, #4]!      @lpc[i] = 0
    SUBS            r7, r7, #1
    BGT             ORDER_LOOP
ORDER_LOOPEND:
    STR             r8, [r14, #4]       @lpc[i] = 0
    BIC             r4, r4, #3
    ADD             r4, r4, #4          @order = ( (order & 0xfffffffc) +4 )


FILTER_LOOP:
    LDR             r1, [sp, #240]      @scaleSpec
@filtering loop here
    CMP             r2, #1              @ inc =1 or -1
    MOV             r7, r4              @loop_count
    BNE             NEG_INC

    LDR             r8   , [r0]         @r8 =*spectrum
    SUBS            r7   , r7   , #1
    MOV             r8, r8, lsl r1
    MOV             r9, r8, asr r1
    MOV             r8   , r8   , lsl r6
    STR             r8   , [r12]        @state[0] = sp[top]
    STR             r9, [r0], #4
    BEQ             FILTER_LOOP2
FILTER_LOOP1:                           @siva 16 times loop run
    LDR             r8   , [r0]         @r8 =*spectrum
    SUB             r5   , r4  , r7     @
    MOV             r5   , r5  , lsl #2
    MOV             r11  , #0           @accu = 0
    ADD             r14, r12, r5        @state[j]
INNER_LOOP1:
    LDR             r10  , [r14, #-4]   @state[j-1]
    LDR             r9   , [r3  , r5]   @lpc[j]
    SUBS            r5   , r5   , #4

    MOV             r2, #0
    SMLAL           r2   , r11, r10, r9
    STR             r10  , [r14], #-4   @state[j] = state[j - 1]
    BGT             INNER_LOOP1

    MOV             r8, r8, lsl r1
    SUB             r8    , r8    , r11, lsl #1
    MOV             r9, r8, asr r1
    STR             r9   , [r0], #4     @*spectrum = y@
    SUBS            r7   , r7   , #1    @i--
    MOV             r8   , r8   , lsl r6
    STR             r8   , [r12]        @state[0]
    BGT             FILTER_LOOP1



@inc=1,order=4



FILTER_LOOP2:
    LDR             R1, [sp]            @size
    ADD             R8, R3, #4

    SUBS            R7   , R1  , r4     @size-order
    BEQ             EXIT

    LDR             R1, [sp, #240]      @scaleSpec

    MOV             R5   , R4  , LSL #2 @count for inner loop = order
    VLD1.32         {D10, D11}, [R8]!   @lpc[j]
    MOV             R14, #0
    VLD1.32         {D12, D13}, [R12]!  @state[j - 1]

    CMP             R4, #4
    VLD1.32         {D18, D19}, [R8]!
    BEQ             ORDER4
    VLD1.32         {D22, D23}, [R12]!

    CMP             R4, #8
    VLD1.32         {D20, D21}, [R8]!
    BEQ             ORDER8
    CMP             R4, #12
    VLD1.32         {D24, D25}, [R12]!
    BEQ             ORDER12

    VLD1.32         {D26, D27}, [R8]!
    CMP             R4, #16
    VLD1.32         {D28, D29}, [R12]!
    BEQ             ORDER16             @order16 added

    VLD1.32         {D4, D5}, [R8]!
    CMP             R4, #20
    VLD1.32         {D8, D9}, [R12]!
    BEQ             ORDER20             @order20 added

ORDER4:
    LDR             r8   , [r0]         @r8 = y = *spectrum
OUTER_LOOP2_4:

    VDUP.32         Q1, R14             @Q1= accu = 0

    VMLAL.S32       Q1, D10, D12

    VMLAL.S32       Q1, D11, D13

    MOV             r8, r8, lsl r1      @y = (*spectrum) << scaleSpec
    SUBS            r7   , r7   , #1

    VADD.I64        D6, D2, D3

    VSHR.S64        D6, #32             @acc1=acc>>32 @acc = mac32_tns_neon(state[j - 1],lpc[j],acc, temp_lo)@
    @VMOV R11,D6[0]
    VST1.32         D6[0], [SP]
    LDR             R11, [SP]

    SUB             r8   , r8   , r11, lsl #1 @y=sub32(y,(acc<<1))
    MOV             r2   , r8   , lsl r6 @ shl32(y, shift_value)

    MOV             r9, r8, asr r1
    VMOV.32         D15[1], R2          @state[0]


    STR             r9   , [r0], #4     @*spectrum = y@
    VEXT.32         Q6, Q7, Q6, #3
    LDRGT           r8   , [r0]         @r8 = y = *spectrum

    BGT             OUTER_LOOP2_4

    B               EXIT
ORDER8:

    LDR             r8   , [r0]         @r8 = y = *spectrum

OUTER_LOOP2_8:

    VDUP.32         Q1, R14             @Q1= accu = 0



    VMLAL.S32       Q1, D10, D12
    VMLAL.S32       Q1, D11, D13
    VMLAL.S32       Q1, D22, D18
    VMLAL.S32       Q1, D23, D19

    MOV             r8, r8, lsl r1      @y = (*spectrum) << scaleSpec
    VEXT.32         Q11, Q6, Q11, #3
    SUBS            r7   , r7   , #1

    VADD.I64        D6, D2, D3
    VSHR.S64        D6, #32             @acc = mac32_tns_neon(state[j - 1],lpc[j],acc, temp_lo)@

    @VMOV R11,D6[0]
    VST1.32         D6[0], [SP]
    LDR             R11, [SP]
    SUB             r8   , r8   , r11, lsl #1 @y=sub32(y,(acc<<1))

    MOV             r2   , r8   , lsl r6 @ shl32(y, shift_value)

    MOV             r9, r8, asr r1
    VMOV.32         D15[1], R2          @state[0]

    STR             r9   , [r0], #4     @*spectrum = y@
    VEXT.32         Q6, Q7, Q6, #3
    LDRGT           r8   , [r0]         @r8 = y = *spectrum


    BGT             OUTER_LOOP2_8

    B               EXIT




ORDER12:
    LDR             r8   , [r0]         @r8 = y = *spectrum
OUTER_LOOP2_12:


    VDUP.32         Q1, R14             @Q1= accu = 0


    VMLAL.S32       Q1, D10, D12
    VMLAL.S32       Q1, D11, D13
    VMLAL.S32       Q1, D22, D18
    VMLAL.S32       Q1, D23, D19
    VMLAL.S32       Q1, D24, D20
    VMLAL.S32       Q1, D25, D21

    MOV             r8, r8, lsl r1      @y = (*spectrum) << scaleSpec
    VEXT.32         Q12, Q11, Q12, #3
    SUBS            r7   , r7   , #1

    VADD.I64        D6, D2, D3
    VEXT.32         Q11, Q6, Q11, #3
    VSHR.S64        D6, #32

    @VMOV R11,D6[0]
    VST1.32         D6[0], [SP]
    LDR             R11, [SP]
    SUB             r8   , r8   , r11, lsl #1 @y=sub32(y,(acc<<1))

    MOV             r2   , r8   , lsl r6 @ shl32(y, shift_value)


    MOV             r9, r8, asr r1
    VMOV.32         D15[1], R2          @state[0]


    STR             r9   , [r0], #4     @*spectrum = y@
    VEXT.32         Q6, Q7, Q6, #3
    LDRGT           r8   , [r0]         @r8 = y = *spectrum

    BGT             OUTER_LOOP2_12

    B               EXIT


ORDER16:
    LDR             r8   , [r0]         @r8 = y = *spectrum
OUTER_LOOP2_16:


    VDUP.32         Q1, R14             @Q1= accu = 0


    VMLAL.S32       Q1, D10, D12
    VMLAL.S32       Q1, D11, D13
    VMLAL.S32       Q1, D22, D18
    VMLAL.S32       Q1, D23, D19
    VMLAL.S32       Q1, D24, D20
    VMLAL.S32       Q1, D25, D21
    VMLAL.S32       Q1, D28, D26        @
    VMLAL.S32       Q1, D29, D27        @ @order16

    VEXT.32         Q14, Q12, Q14, #3
    MOV             r8, r8, lsl r1      @y = (*spectrum) << scaleSpec
    VEXT.32         Q12, Q11, Q12, #3
    SUBS            r7   , r7   , #1

    VADD.I64        D6, D2, D3
    VEXT.32         Q11, Q6, Q11, #3
    VSHR.S64        D6, #32

    @VMOV R11,D6[0]
    VST1.32         D6[0], [SP]
    LDR             R11, [SP]
    SUB             r8   , r8   , r11, lsl #1 @y=sub32(y,(acc<<1))

    MOV             r2   , r8   , lsl r6 @ shl32(y, shift_value)


    MOV             r9, r8, asr r1
    VMOV.32         D15[1], R2          @state[0]


    STR             r9   , [r0], #4     @*spectrum = y@
    VEXT.32         Q6, Q7, Q6, #3
    LDRGT           r8   , [r0]         @r8 = y = *spectrum

    BGT             OUTER_LOOP2_16

    B               EXIT

ORDER20:
    LDR             r8   , [r0]         @r8 = y = *spectrum
OUTER_LOOP2_20:


    VDUP.32         Q1, R14             @Q1= accu = 0


    VMLAL.S32       Q1, D10, D12
    VMLAL.S32       Q1, D11, D13
    VMLAL.S32       Q1, D22, D18
    VMLAL.S32       Q1, D23, D19
    VMLAL.S32       Q1, D24, D20
    VMLAL.S32       Q1, D25, D21
    VMLAL.S32       Q1, D28, D26        @
    VMLAL.S32       Q1, D29, D27        @  @order16
    VMLAL.S32       Q1, D8, D4          @order20
    VMLAL.S32       Q1, D9, D5          @order20


    VEXT.32         Q4, Q14, Q4, #3     @  @for order20
    VEXT.32         Q14, Q12, Q14, #3   @   @for order16
    MOV             r8, r8, lsl r1      @y = (*spectrum) << scaleSpec
    VEXT.32         Q12, Q11, Q12, #3   @order12
    SUBS            r7   , r7   , #1

    VADD.I64        D6, D2, D3
    VEXT.32         Q11, Q6, Q11, #3    @order8
    VSHR.S64        D6, #32

    @VMOV R11,D6[0]
    VST1.32         D6[0], [SP]
    LDR             R11, [SP]
    SUB             r8   , r8   , r11, lsl #1 @y=sub32(y,(acc<<1))

    MOV             r2   , r8   , lsl r6 @ shl32(y, shift_value)


    MOV             r9, r8, asr r1
    VMOV.32         D15[1], R2          @state[0]


    STR             r9   , [r0], #4     @*spectrum = y@
    VEXT.32         Q6, Q7, Q6, #3
    LDRGT           r8   , [r0]         @r8 = y = *spectrum

    BGT             OUTER_LOOP2_20

    B               EXIT

NEG_INC:
@ filtering loop for inc = -1

    LDR             r8   , [r0]         @r8 =*spectrum
    SUBS            r7   , r7   , #1
    MOV             r8, r8, lsl r1
    MOV             r9, r8, asr r1
    MOV             r8   , r8   , lsl r6
    STR             r8   , [r12]        @state[0] = sp[top]
    STR             r9, [r0], #-4
    BEQ             NEGFILTER_LOOP2
NEGFILTER_LOOP1:
    LDR             r8   , [r0]         @r8 =*spectrum
    SUB             r5   , r4  , r7     @
    MOV             r5   , r5  , lsl #2
    MOV             r11  , #0           @accu = 0
    ADD             r14, r12, r5        @state[j]
NEGINNER_LOOP1:
    LDR             r10  , [r14, #-4]   @state[j-1]
    LDR             r9   , [r3  , r5]   @lpc[j]
    SUBS            r5   , r5   , #4

    MOV             r2, #0
    SMLAL           r2   , r11, r10, r9
    STR             r10  , [r14], #-4   @state[j] = state[j - 1]
    BGT             NEGINNER_LOOP1

    MOV             r8, r8, lsl r1
    SUB             r8    , r8    , r11, lsl #1
    MOV             r9, r8, asr r1
    STR             r9   , [r0], #-4    @*spectrum = y@
    SUBS            r7   , r7   , #1    @i--
    MOV             r8   , r8   , lsl r6
    STR             r8   , [r12]        @state[0]
    BGT             NEGFILTER_LOOP1

NEGFILTER_LOOP2:
    LDR             R1, [sp]            @size
    SUBS            R7   , R1  , r4     @size-order
    BEQ             EXIT

    ADD             R8, R3, #4

    MOV             R14, #0
    VLD1.32         {D10, D11}, [R8]!   @lpc[j]
    MOV             R5   , R4  , LSL #2 @count for inner loop = order

    LDR             R1, [sp, #240]      @scaleSpec

    CMP             R4, #4
    VLD1.32         {D12, D13}, [R12]!  @state[j - 1]
    BEQ             NEGORDER4

    VLD1.32         {D18, D19}, [R8]!
    CMP             R4, #8

    VLD1.32         {D22, D23}, [R12]!
    BEQ             NEGORDER8

    VLD1.32         {D20, D21}, [R8]!
    CMP             R4, #12

    VLD1.32         {D24, D25}, [R12]!
    BEQ             NEGORDER12





NEGORDER4:
    LDR             r8   , [r0]         @r8 = y = *spectrum

NEGOUTER_LOOP2_4:

    VDUP.32         Q1, R14             @Q1= accu = 0


    VMLAL.S32       Q1, D10, D12
    VMLAL.S32       Q1, D11, D13
    MOV             r8, r8, lsl r1      @y = (*spectrum) << scaleSpec
    SUBS            r7   , r7   , #1

    VADD.I64        D6, D2, D3
    VSHR.S64        D6, #32



    @VMOV R11,D6[0]
    VST1.32         D6[0], [SP]
    LDR             R11, [SP]
    SUB             r8   , r8   , r11, lsl #1 @y=sub32(y,(acc<<1))

    MOV             r2   , r8   , lsl r6

    VMOV.32         D15[1], R2
    MOV             r9, r8, asr r1


    STR             r9   , [r0], #-4    @*spectrum = y@
    VEXT.32         Q6, Q7, Q6, #3
    LDRGT           r8   , [r0]         @r8 = y = *spectrum

    BGT             NEGOUTER_LOOP2_4

    B               EXIT
NEGORDER8:

    LDR             r8   , [r0]         @r8 = y = *spectrum

NEGOUTER_LOOP2_8:

    VDUP.32         Q1, R14             @Q1= accu = 0



    VMLAL.S32       Q1, D10, D12
    VMLAL.S32       Q1, D11, D13
    VMLAL.S32       Q1, D22, D18
    VMLAL.S32       Q1, D23, D19

    MOV             r8, r8, lsl r1      @y = (*spectrum) << scaleSpec
    VEXT.32         Q11, Q6, Q11, #3
    SUBS            r7   , r7   , #1

    VADD.I64        D6, D2, D3

    VSHR.S64        D6, #32


    @VMOV R11,D6[0]
    VST1.32         D6[0], [SP]
    LDR             R11, [SP]
    SUB             r8   , r8   , r11, lsl #1 @y=sub32(y,(acc<<1))
    MOV             r2   , r8   , lsl r6 @ shl32(y, shift_value)

    VMOV.32         D15[1], R2          @state[0]
    MOV             r9, r8, asr r1


    STR             r9   , [r0], #-4    @*spectrum = y@
    VEXT.32         Q6, Q7, Q6, #3
    LDRGT           r8   , [r0]         @r8 = y = *spectrum


    BGT             NEGOUTER_LOOP2_8

    B               EXIT




NEGORDER12:
    LDR             r8   , [r0]         @r8 = y = *spectrum

NEGOUTER_LOOP2_12:

    VDUP.32         Q1, R14             @Q1= accu = 0



    VMLAL.S32       Q1, D10, D12
    VMLAL.S32       Q1, D11, D13
    VMLAL.S32       Q1, D22, D18
    VMLAL.S32       Q1, D23, D19
    VMLAL.S32       Q1, D24, D20
    VMLAL.S32       Q1, D25, D21

    MOV             r8, r8, lsl r1      @y = (*spectrum) << scaleSpec
    VEXT.32         Q12, Q11, Q12, #3
    SUBS            r7   , r7   , #1

    VADD.I64        D6, D2, D3
    VEXT.32         Q11, Q6, Q11, #3
    VSHR.S64        D6, #32


    @VMOV R11,D6[0]
    VST1.32         D6[0], [SP]
    LDR             R11, [SP]
    SUB             r8   , r8   , r11, lsl #1 @y=sub32(y,(acc<<1))

    MOV             r2   , r8   , lsl r6 @ shl32(y, shift_value)

    VMOV.32         D15[1], R2          @state[0]
    MOV             r9, r8, asr r1



    STR             r9   , [r0], #-4    @*spectrum = y@
    VEXT.32         Q6, Q7, Q6, #3
    LDRGT           r8   , [r0]         @r8 = y = *spectrum


    BGT             NEGOUTER_LOOP2_12


EXIT:
    ADD             sp, sp , #128
    vpop            {d8-d15}
    LDMFD           r13!, {r4 - r12, r15}
