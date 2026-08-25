.text
.p2align 2
.global DSP_fft32x16_dit

DSP_fft32x16_dit:

    STMFD           sp!, {r4-r12, r14}
    VPUSH           {D8-D15}

@**************Variables Vs Registers*************************
@   r0 = *ptr_w
@   r1 = npoints
@   r2 = ptr_x  and
@   r3 = ptr_y
@   r4 = pbit_rev_1024  and pdigRevTable
@   r5 = pbit_rev_512   and p_data1
@   r6 = pbit_rev_128   and p_data2
@   r7 = pbit_rev_32   and p_data3
@   r8 = power    and nstages_4
@   r9 = stage_1_count
@   r10 = first_stage (8 or 4)
@   r11 = p_data4
@   r12 = bit reverse value


@        LDR      r4,[sp,#0x68]
    LDR             r5, [sp, #0x68]
    LDR             r6, [sp, #0x68+4]
    LDR             r7, [sp, #0x68+8]


@ These conditions can be optimised to lesser number
@************************************************************************************

@COND_1 CMP r1, #0x400      @1024
@   BNE COND_2
@   @MOV    r10, #4         @ because radix 8 first stage is by default
@        MOV    r8, #4
@        B  RADIX_4_FIRST_START

@line 59 "../../algo/aacdec/src/neon_asm/fft32x16ch_neon.s"
COND_2: CMP         r1, #0x200          @512
    BNE             COND_3
    @MOV    r10, #8
    MOV             r8, #3
    MOV             r4, r5
    B               RADIX_8_FIRST_START

COND_3: CMP         r1, #0x100
    BNE             COND_4
    @MOV    r10, #4
    MOV             r8, #3
    MOV             r4, r5
    B               RADIX_4_FIRST_START

COND_4: CMP         r1, #0x80           @128
    BNE             COND_5
    @MOV    r10, #8
    MOV             r8, #2
    MOV             r4, r6
    B               RADIX_8_FIRST_START

COND_5: CMP         r1, #0x40
    BNE             COND_6
    @MOV    r10, #4
    MOV             r8, #2
    MOV             r4, r6
    B               RADIX_4_FIRST_START
COND_6:
    @MOV    r10, #8
    MOV             r8, #1
    MOV             r4, r7
@**********************************************************************************


    @CMP    r10,#4
    @BEQ    RADIX_4_FIRST_START

RADIX_8_FIRST_START:


    LSR             r9 , r1, #5         @ LOOP count for first stage
    LSL             r1, r1, #1

RADIX_8_FIRST_LOOP:

    MOV             r5 , r2
    MOV             r6 , r2
    MOV             r7 , r2
    MOV             r11 , r2

@*************** Register mapping to data ****************************************
@ a_data0_r=q0
@ a_data0_i=q1
@ a_data2_r=q2
@ a_data2_i=q3
@ a_data4_r=q4
@ a_data4_i=q5
@ a_data6_r=q6
@ a_data6_i=q7

@ b_data0_r=q8
@ b_data0_i=q9
@ b_data2_r=q10
@ b_data2_i=q11
@ b_data4_r=q12
@ b_data4_i=q13
@ b_data6_r=q14
@ b_data6_i=q15

@*********************************************************************************


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


    @VHADD.S32  q8, q0, q4              @b_data0_r=vhaddq_s32(a_data0_r_i.val[0],a_data4_r_i.val[0])@
    VADD.I32        q8, q0, q4          @b_data0_r=vhaddq_s32(a_data0_r_i.val[0],a_data4_r_i.val[0])@
    VLD2.32         {d5[0], d7[0]}, [r7] , r1
    ADD             r7, r7, r1
    @VHSUB.S32  q9, q0, q4              @b_data4_r=vhsubq_s32(a_data0_r_i.val[0],a_data4_r_i.val[0])@
    VSUB.I32        q9, q0, q4          @b_data4_r=vhsubq_s32(a_data0_r_i.val[0],a_data4_r_i.val[0])@
    VLD2.32         {d13[0], d15[0]}, [r7], r1
    SUB             r7, r7, r1, LSL #2



    @VHADD.S32  q0, q1, q5              @b_data0_i=vhaddq_s32(a_data0_r_i.val[1],a_data4_r_i.val[1])@
    VADD.I32        q0, q1, q5          @b_data0_i=vhaddq_s32(a_data0_r_i.val[1],a_data4_r_i.val[1])@
    VLD2.32         {d5[1], d7[1]}, [r11] , r1
    ADD             r11, r11, r1
    @VHSUB.S32  q4, q1, q5              @b_data4_i=vhsubq_s32(a_data0_r_i.val[1],a_data4_r_i.val[1])@
    VSUB.I32        q4, q1, q5          @b_data4_i=vhsubq_s32(a_data0_r_i.val[1],a_data4_r_i.val[1])@
    VLD2.32         {d13[1], d15[1]}, [r11], r1
    SUB             r11, r11, r1, LSL #2



    ADD             r4, r4, #4

    ADD             r5, r5, r1, LSR #1
    ADD             r6, r6, r1, LSR #1
    ADD             r7, r7, r1, LSR #1
    ADD             r11, r11, r1, LSR #1

    @VHADD.S32  q1, q2, q6              @b_data2_r=vhaddq_s32(a_data2_r_i.val[0],a_data6_r_i.val[0])@
    VADD.I32        q1, q2, q6          @b_data2_r=vhaddq_s32(a_data2_r_i.val[0],a_data6_r_i.val[0])@
    VLD2.32         {d28[0], d30[0]}, [r5] , r1 @a_data1_r_i=vld2q_lane_s32(__transfersize(2) p_data1,a_data1_r_i,0)@

    @VHSUB.S32  q5, q2, q6              @b_data6_r=vhsubq_s32(a_data2_r_i.val[0],a_data6_r_i.val[0])@
    VSUB.I32        q5, q2, q6          @b_data6_r=vhsubq_s32(a_data2_r_i.val[0],a_data6_r_i.val[0])@
    VLD2.32         {d20[0], d22[0]}, [r5] , r1 @a_data3_r_i=vld2q_lane_s32(__transfersize(2) p_data1,a_data3_r_i,0)

    @VHADD.S32  q2, q3, q7              @b_data2_i=vhaddq_s32(a_data2_r_i.val[1],a_data6_r_i.val[1])@
    VADD.I32        q2, q3, q7          @b_data2_i=vhaddq_s32(a_data2_r_i.val[1],a_data6_r_i.val[1])@
    VLD2.32         {d24[0], d26[0]}, [r5] , r1 @a_data5_r_i=vld2q_lane_s32(__transfersize(2) p_data1,a_data5_r_i,0)

    @VHSUB.S32  q6, q3, q7              @b_data6_i=vhsubq_s32(a_data2_r_i.val[1],a_data6_r_i.val[1])@
    VSUB.I32        q6, q3, q7          @b_data6_i=vhsubq_s32(a_data2_r_i.val[1],a_data6_r_i.val[1])@
    VLD2.32         {d28[1], d30[1]}, [r6] , r1

    VADD.S32        q3, q9, q6          @c_data4_r=vaddq_s32(b_data4_r,b_data6_i)@
    VLD2.32         {d20[1], d22[1]}, [r6] , r1

    VSUB.S32        q7, q9, q6          @c_data6_r=vsubq_s32(b_data4_r,b_data6_i)@
    VLD2.32         {d24[1], d26[1]}, [r6] , r1

    VSUB.S32        q6, q4, q5          @c_data4_i=vsubq_s32(b_data4_i,b_data6_r)@
    VLD2.32         {d29[0], d31[0]}, [r7] , r1

    VADD.S32        q9, q4, q5          @c_data6_i=vaddq_s32(b_data4_i,b_data6_r)@
    VLD2.32         {d21[0], d23[0]}, [r7] , r1

    VADD.S32        q4, q8, q1          @c_data0_r=vaddq_s32(b_data0_r,b_data2_r)@
    VLD2.32         {d25[0], d27[0]}, [r7] , r1

    VSUB.S32        q5, q8, q1          @c_data2_r=vsubq_s32(b_data0_r,b_data2_r)@
    VLD2.32         {d29[1], d31[1]}, [r11] , r1

    VADD.S32        q8, q0, q2          @c_data0_i=vaddq_s32(b_data0_i,b_data2_i)@
    VLD2.32         {d21[1], d23[1]}, [r11] , r1

    VSUB.S32        q0, q0, q2          @c_data2_i=vsubq_s32(b_data0_i,b_data2_i)@
    VLD2.32         {d25[1], d27[1]}, [r11] , r1


    VPUSH           {q3}                @ VPUSH(c_data4_r, c_data6_r)
    VPUSH           {q7}








    VLD2.32         {d2[0], d4[0]}, [r5], r1 @a_data7_r_i=vld2q_lane_s32(__transfersize(2) p_data1,a_data7_r_i,0)
    @VHADD.S32  q7, q14, q12                @b_data1_r=vhaddq_s32(a_data1_r,a_data5_r)@
    VADD.I32        q7, q14, q12        @b_data1_r=vhaddq_s32(a_data1_r,a_data5_r)@

    VLD2.32         {d2[1], d4[1]}, [r6] , r1
    @VHSUB.S32  q3, q14, q12                @b_data5_r=vhsubq_s32(a_data1_r,a_data5_r)@
    VSUB.I32        q3, q14, q12        @b_data5_r=vhsubq_s32(a_data1_r,a_data5_r)@

    VLD2.32         {d3[0], d5[0]}, [r7] , r1
    @VHADD.S32  q14, q15, q13               @b_data1_i=vhaddq_s32(a_data1_i,a_data5_i)@
    VADD.I32        q14, q15, q13       @b_data1_i=vhaddq_s32(a_data1_i,a_data5_i)@

    VLD2.32         {d3[1], d5[1]}, [r11] , r1
    @VHSUB.S32  q12, q15, q13               @b_data5_i=vhsubq_s32(a_data1_i,a_data5_i)@
    VSUB.I32        q12, q15, q13       @b_data5_i=vhsubq_s32(a_data1_i,a_data5_i)@




    @VHADD.S32  q15, q10,q1             @b_data3_r=vhaddq_s32(a_data3_r,a_data7_r)@
    @VHSUB.S32  q13, q10,q1             @b_data7_r=vhsubq_s32(a_data3_r,a_data7_r)@
    @VHADD.S32  q10, q11, q2                @b_data3_i=vhaddq_s32(a_data3_i,a_data7_i)@
    @VHSUB.S32  q1, q11, q2             @b_data7_i=vhsubq_s32(a_data3_i,a_data7_i)@

    VADD.I32        q15, q10, q1        @b_data3_r=vhaddq_s32(a_data3_r,a_data7_r)@
    VSUB.I32        q13, q10, q1        @b_data7_r=vhsubq_s32(a_data3_r,a_data7_r)@
    VADD.I32        q10, q11, q2        @b_data3_i=vhaddq_s32(a_data3_i,a_data7_i)@
    VSUB.I32        q1, q11, q2         @b_data7_i=vhsubq_s32(a_data3_i,a_data7_i)@



    VADD.S32        q11, q7, q15        @c_data1_r=vaddq_s32(b_data1_r,b_data3_r)@
    VSUB.S32        q2, q7, q15         @c_data3_r=vsubq_s32(b_data1_r,b_data3_r)@
    VADD.S32        q7, q14, q10        @c_data1_i=vaddq_s32(b_data1_i,b_data3_i)@
    VSUB.S32        q15, q14, q10       @c_data3_i=vsubq_s32(b_data1_i,b_data3_i)@

    VADD.S32        q14, q3, q12        @c_data5_r=vaddq_s32(b_data5_r,b_data5_i)@
    VSUB.S32        q10, q3, q12        @c_data5_i=vsubq_s32(b_data5_r,b_data5_i)@
    VADD.S32        q3, q13, q1         @c_data7_r=vaddq_s32(b_data7_r,b_data7_i)@
    VSUB.S32        q12, q13, q1        @c_data7_i=vsubq_s32(b_data7_r,b_data7_i)@

    VADD.S32        q1 , q14, q12       @b_data5_r=vaddq_s32(c_data7_i,c_data5_r)@
    VSUB.S32        q13, q14, q12       @b_data7_i=vsubq_s32(c_data5_r,c_data7_i)@
    VSUB.S32        q12, q3, q10        @b_data5_i=vsubq_s32(c_data7_r,c_data5_i)@

    VUZP.16         d2, d3              @ D0 = b_data5_r_low, D1= b_data5_r_high
    VADD.S32        q14, q3, q10        @b_data7_r=vaddq_s32(c_data5_i,c_data7_r)@

    VUZP.16         d26, d27
    VADD.S32        q3, q4, q11         @b_data0_r=vaddq_s32(c_data0_r,c_data1_r)@

    VUZP.16         d24, d25
    VSUB.S32        q10, q4, q11        @b_data1_r=vsubq_s32(c_data0_r,c_data1_r)@

    VUZP.16         d28, d29
    VADD.S32        q4, q8, q7          @b_data0_i=vaddq_s32(c_data0_i,c_data1_i)@

    LDR             r14, = 0x5a82

    VSUB.S32        q11, q8, q7         @b_data1_i=vsubq_s32(c_data0_i,c_data1_i)@

    VADD.S32        q8, q5, q15         @b_data2_r=vaddq_s32(c_data2_r,c_data3_i)@
    VSUB.S32        q7, q5, q15         @b_data3_r=vsubq_s32(c_data2_r,c_data3_i)@
    VSUB.S32        q5, q0, q2          @b_data2_i=vsubq_s32(c_data2_i,c_data3_r)@
    VADD.S32        q15, q0, q2         @b_data3_i=vaddq_s32(c_data2_i,c_data3_r)@

    VPOP            {q0}
    VPOP            {q2}
    VPUSH           {q3-q4}
    VPUSH           {q10}




@********************************************************************
@ b_data5_r = q1       free regs = q3,q4,q5,q7,q8,q10,q11
@ b_data5_i = q12
@ b_data7_r = q14
@ b_data7_i = q13

@ c_data4_r = q2
@ c_data4_i = q6
@ c_data6_r = q0
@ c_data6_i = q9
@********************************************************************


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

    VADD.S32        q5, q2, q4          @q5=b_data7_i
    VSUB.S32        q7, q2, q4          @q7=b_data4_r

    VADD.S32        q8, q6, q3          @q10 = b_data4_i
    VSUB.S32        q6, q6, q3          @q11 = b_data7_r






    VSHR.S32        q13, q13, #15
    VSHR.S32        q14, q14, #15

    VQDMLAL.S16     q13, d25, d20
    VQDMLAL.S16     q14, d3, d20

    VPOP            {q1}
    VPOP            {q10}

    VADD.S32        q2, q0, q13         @q2 = b_data5_i
    VSUB.S32        q4, q0, q13         @q4 = b_data6_r

    VADD.S32        q11, q9, q14        @q6 = b_data6_i
    VSUB.S32        q3, q9, q14         @q8 = b_data5_r




    VPOP            {q14}
    VPOP            {q9}
    VPOP            {q0}
    VPOP            {q12, q13}





@**************regs maping************
@b_data0_r = q12
@b_data0_i = q13
@b_data1_r = q0
@b_data1_i = q1

@b_data2_r = q9
@b_data2_i = q10
@b_data3_r = q14
@b_data3_i = q15

@b_data4_r = q7
@b_data4_i = q8
@b_data5_r = q3
@b_data5_i = q2

@b_data6_r = q4
@b_data6_i = q11
@b_data7_r = q6
@b_data7_i = q5
@******************************************

@shifts added (as dual simd instrn)

    VTRN.32         q12, q5
@line 455 "../../algo/aacdec/src/neon_asm/fft32x16ch_neon.s"
    VSHL.S32        q12, q12, #3        @ch
    VTRN.32         q9, q2
    VSHL.S32        q5, q5, #3          @ch

    VSHL.S32        q9, q9, #3          @ch
    VTRN.32         q0, q7
    VSHL.S32        q2, q2, #3          @ch

    VSHL.S32        q0, q0, #3          @ch
    VTRN.32         q14, q4
    VSHL.S32        q7, q7, #3          @ch

    VSHL.S32        q14, q14, #3        @ch
    VTRN.32         q13, q6
    VSHL.S32        q4, q4, #3          @ch

    VSHL.S32        q13, q13, #3        @ch
    VTRN.32         q10, q3
    VSHL.S32        q6, q6, #3          @ch

    VSHL.S32        q10, q10, #3        @ch
    VTRN.32         q1, q8
    VSHL.S32        q3, q3, #3          @ch

    VSHL.S32        q1, q1, #3          @ch
    VTRN.32         q15, q11
    VSHL.S32        q8, q8, #3          @ch

    VSHL.S32        q15, q15, #3        @ch
    VSWP            d18, d25

    VSHL.S32        q11, q11, #3        @ch
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




@************************************RADIX 4 FIRST STAGE**********************************

RADIX_4_FIRST_START:


    LSR             r9 , r1, #4         @ LOOP count for first stage
    LSL             r1, r1, #1

RADIX_4_LOOP:

    MOV             r5 , r2
    MOV             r6 , r2
    MOV             r7 , r2
    MOV             r11 , r2

@*************** Register mapping to data ****************************************
@ a_data0_r=q0
@ a_data0_i=q1
@ a_data1_r=q2
@ a_data1_i=q3
@ a_data2_r=q4
@ a_data2_i=q5
@ a_data3_r=q6
@ a_data4_i=q7


@*********************************************************************************


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
    VADD.S32        q8, q0, q4          @b_data0_r=vaddq_s32(a_data0_r,a_data2_r)@
    VLD2.32         {d5[0], d7[0]}, [r7] , r1
    ADD             r7, r7, r1
    VADD.S32        q9, q1, q5          @b_data0_i=vaddq_s32(a_data0_i,a_data2_i)@
    VLD2.32         {d13[0], d15[0]}, [r7], r1



    SUB             r11, r11, r1, LSL #1
    VSUB.S32        q10, q0, q4         @b_data2_r=vsubq_s32(a_data0_r,a_data2_r)@
    VLD2.32         {d5[1], d7[1]}, [r11] , r1
    ADD             r11, r11, r1
    VSUB.S32        q11, q1, q5         @b_data2_i=vsubq_s32(a_data0_i,a_data2_i)@
    VLD2.32         {d13[1], d15[1]}, [r11], r1


    ADD             r4, r4, #4

    VADD.S32        q12, q2, q6         @b_data1_r=vaddq_s32(a_data1_r,a_data3_r)@
    VADD.S32        q13, q3, q7         @b_data1_i=vaddq_s32(a_data1_i,a_data3_i)@
    VSUB.S32        q14, q2, q6         @b_data3_r=vsubq_s32(a_data1_r,a_data3_r)@
    VSUB.S32        q15, q3, q7         @b_data3_i=vsubq_s32(a_data1_i,a_data3_i)@

    VADD.S32        q0, q8, q12         @a_data0_r=vaddq_s32(b_data0_r,b_data1_r)@
    VADD.S32        q1, q9, q13         @a_data0_i=vaddq_s32(b_data0_i,b_data1_i)@
    VSUB.S32        q2, q8, q12         @a_data1_r=vsubq_s32(b_data0_r,b_data1_r)@
    VSUB.S32        q3, q9, q13         @a_data1_i=vsubq_s32(b_data0_i,b_data1_i)@

    VADD.S32        q4, q10, q15        @a_data2_r=vaddq_s32(b_data2_r,b_data3_i)@
    VSUB.S32        q5, q11, q14        @a_data2_i=vsubq_s32(b_data2_i,b_data3_r)@
    VADD.S32        q7, q11, q14        @a_data3_r=vaddq_s32(b_data2_i,b_data3_r)@
    VSUB.S32        q6, q10, q15        @a_data3_i=vsubq_s32(b_data2_r,b_data3_i)@


@shifts added

    VTRN.32         q0, q4

    VSHL.S32        q0, q0, #2          @ch
    VTRN.32         q2, q6
    VSHL.S32        q4, q4, #2          @ch

    VSHL.S32        q2, q2, #2          @ch
    VTRN.32         q1, q5              @ch
    VSHL.S32        q6, q6, #2          @ch

    VSHL.S32        q1, q1, #2          @ch
    VTRN.32         q3, q7              @ch
    VSHL.S32        q5, q5, #2          @ch

    VSHL.S32        q3, q3, #2          @ch
    VSWP            d4, d1

    VSHL.S32        q7, q7, #2          @ch
    VSWP            d12, d9

    @VTRN.32        q1, q5
    @VTRN.32        q3, q7
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



@********************************END OF RADIX 4 FIRST STAGE*******************************

@*************** register assignment after first radix 8 stage****************************
@ r1 = npoints
@ r0 = *ptr_w
@ r3 = *ptr_y
@ r8 = nstages_4
@ free regs r2, r4,r5,r6,r7,r9,r10,r11,r12
@ r2 = j
@ r4 = node_spacing
@ r5 = del
@ r6 = in_loop_count
@ r7 = middle_loop_count (del*node_spacing)
@ r9 = p_twiddle_factors
@ r10= p_twiddle_factors  and inner loop counter
@ r11=
@ r12=
@ r14= *data

    PUSH            {r3}

    LSR             r5, r5, #2

OUTER_LOOP_R4:

    LDR             r14, [sp]
    @MOV    r14,r3
    @LSR    r7,r5,#0    @,#2
    MOV             r7, r5
    MOV             r2, #0
    MOV             r9, r0
    LSL             r12 , r5, #5
MIDDLE_LOOP_R4:


    VLD2.16         {d0[0], d1[0]}, [r9], r2 @cos_1 = d0 , sin_1=d1
    VLD2.16         {d2[0], d3[0]}, [r9], r2 @cos_2 = d2 , sin_2=d3
    ADD             r11, r2, r4, LSL #2
    VLD2.16         {d4[0], d5[0]}, [r9] @cos_3 = d4 , sin_3=d5
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
    VLD4.16         {q5, q6}, [r14], r12 @a_data1_r_l=d10 , a_data1_r_h=d11, a_data1_i_l=d12, a_data1_i_h=d13
    VSHR.S32        q4, q4, #1

    VSHR.U16        d10, d10, #1        @a_data1.val[0]= vreinterpret_s16_u16(vshr_n_u16(vreinterpret_u16_s16(a_data1.val[0]), 1))@
    VLD4.16         {q7, q8}, [r14], r12
    VSHR.U16        d12, d12, #1        @a_data1.val[2]= vreinterpret_s16_u16(vshr_n_u16(vreinterpret_u16_s16(a_data1.val[2]), 1))@

    VMULL.S16       q11, d10, d0        @prod_1r=vmull_s16(a_data1.val[0], cos_1)@
    VMLSL.S16       q11, d12, d1        @prod_1r=vmlsl_s16(prod_1r, a_data1.val[2], sin_1)@
    VLD4.16         {q9, q10}, [r14], r12
    VMULL.S16       q12, d10, d1        @prod_1i=vmull_s16(a_data1.val[0], sin_1)@
    VMLAL.S16       q12, d12, d0        @prod_1i=vmlal_s16(prod_1i, a_data1.val[2], cos_1)@

    VSHR.U16        d14, d14, #1        @a_data2.val[0]=vreinterpret_s16_u16(vshr_n_u16(vreinterpret_u16_s16(a_data2.val[0]), 1))@
    VSHR.U16        d16, d16, #1        @a_data2.val[2]=vreinterpret_s16_u16(vshr_n_u16(vreinterpret_u16_s16(a_data2.val[2]), 1))@

    SUB             r14, r14, r12, LSL #2

    VSHR.U16        d18, d18, #1        @a_data3.val[0]= vreinterpret_s16_u16(vshr_n_u16(vreinterpret_u16_s16(a_data3.val[0]), 1))@
    VSHR.U16        d20, d20, #1        @a_data3.val[2]= vreinterpret_s16_u16(vshr_n_u16(vreinterpret_u16_s16(a_data3.val[2]), 1))@

    VMULL.S16       q13, d14, d2        @prod_2r=vmull_s16(a_data2.val[0], cos_2)@
    VMLSL.S16       q13, d16, d3        @prod_2r=vmlsl_s16(prod_2r, a_data2.val[2], sin_2)@

    VSHR.S32        q11, q11, #15       @a_data1_r=vshrq_n_s32(prod_1r,15)@

    VMULL.S16       q14, d14, d3        @prod_2i=vmull_s16(a_data2.val[0], sin_2)@
    VMLAL.S16       q14, d16, d2        @prod_2i=vmlal_s16(prod_2i, a_data2.val[2], cos_2)@

    VMULL.S16       q15, d18, d4        @prod_3r=vmull_s16(a_data3.val[0], cos_3)@
    VMLSL.S16       q15, d20, d5        @prod_3r=vmlsl_s16(prod_3r, a_data3.val[2], sin_3)@

    VMLAL.S16       q11, d11, d0        @a_data1_r=vmlal_s16(a_data1_r, a_data1.val[1], cos_1)@
    VMLSL.S16       q11, d13, d1        @a_data1_r=vmlsl_s16(a_data1_r, a_data1.val[3], sin_1)@

    VSHR.S32        q12, q12, #15       @a_data1_i=vshrq_n_s32(prod_1i,15)@
    VSHR.S32        q13, q13, #15       @a_data2_r=vshrq_n_s32(prod_2r,15)@
    VSHR.S32        q14, q14, #15       @a_data2_i=vshrq_n_s32(prod_2i,15)@
    VSHR.S32        q15, q15, #15       @a_data3_r=vshrq_n_s32(prod_3r,15)@


    VMLAL.S16       q12, d11, d1        @a_data1_i=vmlal_s16(a_data1_i, a_data1.val[1], sin_1)@
    VMLAL.S16       q12, d13, d0        @a_data1_i=vmlal_s16(a_data1_i, a_data1.val[3], cos_1)@


    VMULL.S16       q5, d18, d5         @prod_3i=vmull_s16(a_data3.val[0], sin_3)@
    VMLAL.S16       q5, d20, d4         @prod_3i=vmlal_s16(prod_3i, a_data3.val[2], cos_3)@


    VMLAL.S16       q13, d15, d2        @a_data2_r=vmlal_s16(a_data2_r, a_data2.val[1], cos_2)@
    VMLSL.S16       q13, d17, d3        @a_data2_r=vmlsl_s16(a_data2_r, a_data2.val[3], sin_2)@

    VMLAL.S16       q14, d15, d3        @a_data2_i=vmlal_s16(a_data2_i, a_data2.val[1], sin_2)@
    VMLAL.S16       q14, d17, d2        @a_data2_i=vmlal_s16(a_data2_i, a_data2.val[3], cos_2)@


    VMLAL.S16       q15, d19, d4        @a_data3_r=vmlal_s16(a_data3_r, a_data3.val[1], cos_3)@
    VMLSL.S16       q15, d21, d5        @a_data3_r=vmlsl_s16(a_data3_r, a_data3.val[3], sin_3)@

    VSHR.S32        q5, q5, #15         @a_data3_i=vshrq_n_s32(prod_3i,15)@

    VMLAL.S16       q5, d19, d5         @a_data3_i=vmlal_s16(a_data3_i, a_data3.val[1], sin_3)@
    VMLAL.S16       q5, d21, d4         @a_data3_i=vmlal_s16(a_data3_i, a_data3.val[3], cos_3)@

@**********if condition******************

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
@****************************************
BYPASS_IF:

    VADD.S32        q6, q3, q13         @b_data0_r=vaddq_s32(a_data0_r,a_data2_r)@
    VADD.S32        q7, q4, q14         @b_data0_i=vaddq_s32(a_data0_i,a_data2_i)@
    VSUB.S32        q3, q3, q13         @b_data2_r=vsubq_s32(a_data0_r,a_data2_r)@
    VSUB.S32        q4, q4, q14         @b_data2_i=vsubq_s32(a_data0_i,a_data2_i)@
    VADD.S32        q8, q11, q15        @b_data1_r=vaddq_s32(a_data1_r,a_data3_r)@
    VADD.S32        q9, q12, q5         @b_data1_i=vaddq_s32(a_data1_i,a_data3_i)@

    VSUB.S32        q15, q11, q15       @b_data3_r=vsubq_s32(a_data1_r,a_data3_r)@
    VSUB.S32        q14, q12, q5        @b_data3_i=vsubq_s32(a_data1_i,a_data3_i)@

@line 882 "../../algo/aacdec/src/neon_asm/fft32x16ch_neon.s"
    VADD.S32        q10, q6, q8         @c_data0_r=vaddq_s32(b_data0_r,b_data1_r)@
    VADD.S32        q11, q7, q9         @c_data0_i=vaddq_s32(b_data0_i,b_data1_i)@
    VADD.S32        q12, q3, q14        @c_data2_r=vaddq_s32(b_data2_r,b_data3_i)@
    VSUB.S32        q13, q4, q15        @c_data2_i=vsubq_s32(b_data2_i,b_data3_r)@

    VSUB.S32        q6, q6, q8          @c_data1_r=vsubq_s32(b_data0_r,b_data1_r)@
    VST2.32         {q10, q11}, [r14], r12 @ storing (c_data0_r,c_data0_i)
    VSUB.S32        q7, q7, q9          @c_data1_i=vsubq_s32(b_data0_i,b_data1_i)@

    VSUB.S32        q8, q3, q14         @c_data3_i=vsubq_s32(b_data2_r,b_data3_i)@
    VST2.32         {q12, q13}, [r14], r12 @ storing (c_data2_r,c_data2_i)
    VADD.S32        q9, q4, q15         @c_data3_r=vaddq_s32(b_data2_i,b_data3_r)@


    VST2.32         {q6, q7}, [r14], r12 @ storing (c_data1_r,c_data1_i)
    VST2.32         {q8, q9}, [r14], r12 @ storing (c_data3_i,c_data3_r)




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
    VPOP            {D8-D15}
    LDMFD           sp!, {r4-r12, r15}


