.text
.p2align 2
.global ixheaacd_fft32x32_ld2_armv7

ixheaacd_fft32x32_ld2_armv7:

    STMFD           sp!, {r4-r12, r14}

    @DIT Radix-4 FFT First Stage
    @First Butterfly
    MOV             r0, r2
    MOV             r1, r3
    LDR             r2, [r0]            @x_0 = x[0 ]
    LDR             r3, [r0, #32]       @x_2 = x[8 ]
    LDR             r4, [r0, #64]       @x_4 = x[16]
    LDR             r5, [r0, #96]       @x_6 = x[24]
    ADD             r6, r2, r4          @xh0_0 = x_0 + x_4
    SUB             r7, r2, r4          @xl0_0 = x_0 - x_4
    ADD             r8, r3, r5          @xh0_1 = x_2 + x_6
    SUB             r9, r3, r5          @xl0_1 = x_2 - x_6

    LDR             r2, [r0, #4]        @x_1 = x[0 +1]
    LDR             r3, [r0, #36]       @x_3 = x[8 +1]
    LDR             r4, [r0, #68]       @x_5 = x[16+1]
    LDR             r5, [r0, #100]      @x_7 = x[24+1]
    ADD             r10, r2, r4         @xh1_0 = x_1 + x_5
    SUB             r11, r2, r4         @xl1_0 = x_1 - x_5
    ADD             r12, r3, r5         @xh1_1 = x_3 + x_7
    SUB             r14, r3, r5         @xl1_1 = x_3 - x_7

    ADD             r2, r6, r8          @n00 = xh0_0 + xh0_1
    ADD             r3, r7, r14         @n10 = xl0_0 + xl1_1
    SUB             r4, r6, r8          @n20 = xh0_0 - xh0_1
    SUB             r5, r7, r14         @n30 = xl0_0 - xl1_1
    STR             r2, [r0]            @x[0 ] = n00
    STR             r3, [r0, #32]       @x[8 ] = n10
    STR             r4, [r0, #64]       @x[16] = n20
    STR             r5, [r0, #96]       @x[24] = n30

    ADD             r2, r10, r12        @n01 = xh1_0 + xh1_1
    SUB             r3, r11, r9         @n11 = xl1_0 - xl0_1
    SUB             r4, r10, r12        @n21 = xh1_0 - xh1_1
    ADD             r5, r11, r9         @n31 = xl1_0 + xl0_1
    STR             r2, [r0, #4]        @x[1   ] = n01
    STR             r3, [r0, #36]       @x[8+1 ] = n11
    STR             r4, [r0, #68]       @x[16+1] = n21
    STR             r5, [r0, #100]      @x[24+1] = n31

    @Second Butterfly
    LDR             r2, [r0, #8]        @x_0 = x[2 ]
    LDR             r3, [r0, #40]       @x_2 = x[10]
    LDR             r4, [r0, #72]       @x_4 = x[18]
    LDR             r5, [r0, #104]      @x_6 = x[26]
    ADD             r6, r2, r4          @xh0_0 = x_0 + x_4
    SUB             r7, r2, r4          @xl0_0 = x_0 - x_4
    ADD             r8, r3, r5          @xh0_1 = x_2 + x_6
    SUB             r9, r3, r5          @xl0_1 = x_2 - x_6

    LDR             r2, [r0, #12]       @x_1 = x[2 +1]
    LDR             r3, [r0, #44]       @x_3 = x[10+1]
    LDR             r4, [r0, #76]       @x_5 = x[18+1]
    LDR             r5, [r0, #108]      @x_7 = x[26+1]
    ADD             r10, r2, r4         @xh1_0 = x_1 + x_5
    SUB             r11, r2, r4         @xl1_0 = x_1 - x_5
    ADD             r12, r3, r5         @xh1_1 = x_3 + x_7
    SUB             r14, r3, r5         @xl1_1 = x_3 - x_7

    ADD             r2, r6, r8          @n00 = xh0_0 + xh0_1
    ADD             r3, r7, r14         @n10 = xl0_0 + xl1_1
    SUB             r4, r6, r8          @n20 = xh0_0 - xh0_1
    SUB             r5, r7, r14         @n30 = xl0_0 - xl1_1
    STR             r2, [r0, #8]        @x[2 ] = n00
    STR             r3, [r0, #40]       @x[10] = n10
    STR             r4, [r0, #72]       @x[18] = n20
    STR             r5, [r0, #104]      @x[26] = n30

    ADD             r2, r10, r12        @n01 = xh1_0 + xh1_1
    SUB             r3, r11, r9         @n11 = xl1_0 - xl0_1
    SUB             r4, r10, r12        @n21 = xh1_0 - xh1_1
    ADD             r5, r11, r9         @n31 = xl1_0 + xl0_1
    STR             r2, [r0, #12]       @x[2 +1] = n01
    STR             r3, [r0, #44]       @x[10+1] = n11
    STR             r4, [r0, #76]       @x[18+1] = n21
    STR             r5, [r0, #108]      @x[26+1] = n31

    @Third Butterfly
    LDR             r2, [r0, #16]       @x_0 = x[4 ]
    LDR             r3, [r0, #48]       @x_2 = x[12]
    LDR             r4, [r0, #80]       @x_4 = x[20]
    LDR             r5, [r0, #112]      @x_6 = x[28]
    ADD             r6, r2, r4          @xh0_0 = x_0 + x_4
    SUB             r7, r2, r4          @xl0_0 = x_0 - x_4
    ADD             r8, r3, r5          @xh0_1 = x_2 + x_6
    SUB             r9, r3, r5          @xl0_1 = x_2 - x_6

    LDR             r2, [r0, #20]       @x_1 = x[4 +1]
    LDR             r3, [r0, #52]       @x_3 = x[12+1]
    LDR             r4, [r0, #84]       @x_5 = x[20+1]
    LDR             r5, [r0, #116]      @x_7 = x[28+1]
    ADD             r10, r2, r4         @xh1_0 = x_1 + x_5
    SUB             r11, r2, r4         @xl1_0 = x_1 - x_5
    ADD             r12, r3, r5         @xh1_1 = x_3 + x_7
    SUB             r14, r3, r5         @xl1_1 = x_3 - x_7

    ADD             r2, r6, r8          @n00 = xh0_0 + xh0_1
    ADD             r3, r7, r14         @n10 = xl0_0 + xl1_1
    SUB             r4, r6, r8          @n20 = xh0_0 - xh0_1
    SUB             r5, r7, r14         @n30 = xl0_0 - xl1_1
    STR             r2, [r0, #16]       @x[4 ] = n00
    STR             r3, [r0, #48]       @x[12] = n10
    STR             r4, [r0, #80]       @x[20] = n20
    STR             r5, [r0, #112]      @x[28] = n30

    ADD             r2, r10, r12        @n01 = xh1_0 + xh1_1
    SUB             r3, r11, r9         @n11 = xl1_0 - xl0_1
    SUB             r4, r10, r12        @n21 = xh1_0 - xh1_1
    ADD             r5, r11, r9         @n31 = xl1_0 + xl0_1
    STR             r2, [r0, #20]       @x[4 +1] = n01
    STR             r3, [r0, #52]       @x[12+1] = n11
    STR             r4, [r0, #84]       @x[20+1] = n21
    STR             r5, [r0, #116]      @x[28+1] = n31

    @Fourth Butterfly
    LDR             r2, [r0, #24]       @x_0 = x[6 ]
    LDR             r3, [r0, #56]       @x_2 = x[14]
    LDR             r4, [r0, #88]       @x_4 = x[22]
    LDR             r5, [r0, #120]      @x_6 = x[30]
    ADD             r6, r2, r4          @xh0_0 = x_0 + x_4
    SUB             r7, r2, r4          @xl0_0 = x_0 - x_4
    ADD             r8, r3, r5          @xh0_1 = x_2 + x_6
    SUB             r9, r3, r5          @xl0_1 = x_2 - x_6

    LDR             r2, [r0, #28]       @x_1 = x[6 +1]
    LDR             r3, [r0, #60]       @x_3 = x[14+1]
    LDR             r4, [r0, #92]       @x_5 = x[22+1]
    LDR             r5, [r0, #124]      @x_7 = x[30+1]
    ADD             r10, r2, r4         @xh1_0 = x_1 + x_5
    SUB             r11, r2, r4         @xl1_0 = x_1 - x_5
    ADD             r12, r3, r5         @xh1_1 = x_3 + x_7
    SUB             r14, r3, r5         @xl1_1 = x_3 - x_7

    ADD             r2, r6, r8          @n00 = xh0_0 + xh0_1
    ADD             r3, r7, r14         @n10 = xl0_0 + xl1_1
    SUB             r4, r6, r8          @n20 = xh0_0 - xh0_1
    SUB             r5, r7, r14         @n30 = xl0_0 - xl1_1
    STR             r2, [r0, #24]       @x[6 ] = n00
    STR             r3, [r0, #56]       @x[14] = n10
    STR             r4, [r0, #88]       @x[22] = n20
    STR             r5, [r0, #120]      @x[30] = n30

    ADD             r2, r10, r12        @n01 = xh1_0 + xh1_1
    SUB             r3, r11, r9         @n11 = xl1_0 - xl0_1
    SUB             r4, r10, r12        @n21 = xh1_0 - xh1_1
    ADD             r5, r11, r9         @n31 = xl1_0 + xl0_1
    STR             r2, [r0, #28]       @x[6 +1] = n01
    STR             r3, [r0, #60]       @x[14+1] = n11
    STR             r4, [r0, #92]       @x[22+1] = n21
    STR             r5, [r0, #124]      @x[30+1] = n31


    @DIT Radix-4 FFT Second Stage
    @First Butterfly
    LDR             r2, [r0]            @inp_0qr = x[0]
    LDR             r3, [r0, #8]        @inp_1qr = x[2]
    LDR             r4, [r0, #16]       @inp_2qr = x[4]
    LDR             r5, [r0, #24]       @inp_3qr = x[6]
    ADD             r6, r2, r4          @sum_0qr  = mul_0qr + mul_2qr
    SUB             r7, r2, r4          @sum_1qr  = mul_0qr - mul_2qr
    ADD             r8, r3, r5          @sum_2qr  = mul_1qr + mul_3qr
    SUB             r9, r3, r5          @sum_3qr  = mul_1qr - mul_3qr

    LDR             r2, [r0, #4]        @inp_0qi = x[1]
    LDR             r3, [r0, #12]       @inp_1qi = x[3]
    LDR             r4, [r0, #20]       @inp_2qi = x[5]
    LDR             r5, [r0, #28]       @inp_3qi = x[7]
    ADD             r10, r2, r4         @sum_0qi  = mul_0qi + mul_2qi
    SUB             r11, r2, r4         @sum_1qi  = mul_0qi - mul_2qi
    ADD             r12, r3, r5         @sum_2qi  = mul_1qi + mul_3qi
    SUB             r14, r3, r5         @sum_3qi  = mul_1qi - mul_3qi

    ADD             r2, r6, r8          @sum_0qr + sum_2qr
    ADD             r3, r7, r14         @sum_1qr + sum_3qi
    SUB             r4, r6, r8          @sum_0qr - sum_2qr
    SUB             r5, r7, r14         @sum_1qr - sum_3qi
    STR             r2, [r1]            @y[0 ] = sum_0qr + sum_2qr
    STR             r3, [r1, #32]       @y[8 ] = sum_1qr + sum_3qi
    STR             r4, [r1, #64]       @y[16] = sum_0qr - sum_2qr
    STR             r5, [r1, #96]       @y[24] = sum_1qr - sum_3qi

    ADD             r2, r10, r12        @sum_0qi + sum_2qi
    SUB             r3, r11, r9         @sum_1qi - sum_3qr
    SUB             r4, r10, r12        @sum_0qi - sum_2qi
    ADD             r5, r11, r9         @sum_1qi + sum_3qr
    STR             r2, [r1, #4]        @y[0 +1] = sum_0qi + sum_2qi
    STR             r3, [r1, #36]       @y[8 +1] = sum_1qi - sum_3qr
    STR             r4, [r1, #68]       @y[16+1] = sum_0qi - sum_2qi
    STR             r5, [r1, #100]      @y[24+1] = sum_1qi + sum_3qr


    @Load twiddle factors
    MOVW            r11, 0X7642
    MOVT            r11, 0X89BE
    MOVW            r12, 0X30FC
    MOVT            r12, 0XCF04
    MOVW            r14, 0X5A83
    MOVT            r14, 0XA57D

    @Second Butterfly
    LDR             r2, [r0, #32]       @mul_0qr = inp_0qr = x[8]
    LDR             r3, [r0, #36]       @mul_0qi = inp_1qr = x[9]

    LDR             r5, [r0, #40]       @inp_1qr = x[10]
    LDR             r6, [r0, #44]       @inp_1qi = x[11]
    SMULWB          r4, r5, r11         @mul_1qr = mpy_16_32_ns( 0x7642 , inp_1qr)
    SMLAWB          r4, r6, r12, r4     @mul_1qr -= mpy_16_32_ns(-0x30FC , inp_1qi)
    SMULWT          r5, r5, r12         @mul_1qi = mpy_16_32_ns(-0x30FC , inp_1qr)

    LDR             r7, [r0, #48]       @inp_2qr = x[12]
    LDR             r8, [r0, #52]       @inp_2qi = x[13]

    @Moved for delay slot
    SMLAWB          r5, r6, r11, r5     @mul_1qi += mpy_16_32_ns( 0x7642 , inp_1qi)

    ADD             r6, r7, r8          @(inp_2qr + inp_2qi)
    SMULWB          r6, r6, r14         @mul_2qr = mpy_16_32_ns(0x5A83 , (inp_2qr + inp_2qi))
    SUB             r7, r8, r7          @(-inp_2qr + inp_2qi)
    SMULWB          r7, r7, r14         @mul_2qi = mpy_16_32_ns(0x5A83 , (-inp_2qr + inp_2qi))

    LDR             r9 , [r0, #56]      @inp_3qr = x[14]
    LDR             r10, [r0, #60]      @inp_3qi = x[15]
    SMULWB          r8, r9 , r12        @mul_3qr = mpy_16_32_ns( 0x30FC , inp_3qr)
    SMLAWB          r8, r10, r11, r8    @mul_3qr -= mpy_16_32_ns(-0x7642 , inp_3qi)@
    SMULWT          r9, r9 , r11        @mul_3qi = mpy_16_32_ns(-0x7642 , inp_3qr)
    SMLAWB          r9, r10, r12, r9    @mul_3qi += mpy_16_32_ns( 0x30FC , inp_3qi)

    ADD             r10, r2, r6, lsl #1 @sum_0qr  = mul_0qr + (mul_2qr << 1)
    SUB             r2 , r2, r6, lsl #1 @sum_1qr  = mul_0qr - (mul_2qr << 1)
    ADD             r6 , r4, r8         @sum_2qr  = mul_1qr + mul_3qr
    SUB             r4 , r4, r8         @sum_3qr  = mul_1qr - mul_3qr

    ADD             r8 , r3, r7, lsl #1 @sum_0qi  = mul_0qi + (mul_2qi << 1)
    SUB             r3 , r3, r7, lsl #1 @sum_1qi  = mul_0qi - (mul_2qi << 1)
    ADD             r7 , r5, r9         @sum_2qi  = mul_1qi + mul_3qi
    SUB             r5 , r5, r9         @sum_3qi  = mul_1qi - mul_3qi

    ADD             r9 , r10, r6, lsl #1 @sum_0qr + (sum_2qr << 1)
    SUB             r10, r10, r6, lsl #1 @sum_0qr - (sum_2qr << 1)
    ADD             r6 , r2 , r5, lsl #1 @sum_1qr + (sum_3qi << 1)
    SUB             r2 , r2 , r5, lsl #1 @sum_1qr - (sum_3qi << 1)
    STR             r9 , [r1, #8]       @y[2 ] = sum_0qr + (sum_2qr << 1)
    STR             r10, [r1, #72]      @y[18] = sum_0qr - (sum_2qr << 1)
    STR             r6 , [r1, #40]      @y[10] = sum_1qr + (sum_3qi << 1)
    STR             r2 , [r1, #104]     @y[26] = sum_1qr - (sum_3qi << 1)

    ADD             r5 , r8 , r7, lsl #1 @sum_0qi + (sum_2qi << 1)
    SUB             r8 , r8 , r7, lsl #1 @sum_0qi - (sum_2qi << 1)
    SUB             r7 , r3 , r4, lsl #1 @sum_1qi - (sum_3qr << 1)
    ADD             r3 , r3 , r4, lsl #1 @sum_1qi + (sum_3qr << 1)
    STR             r5 , [r1, #12]      @y[2 +1] = sum_0qi + (sum_2qi << 1)
    STR             r8 , [r1, #76]      @y[18+1] = sum_0qi - (sum_2qi << 1)
    STR             r7 , [r1, #44]      @y[10+1] = sum_1qi - (sum_3qr << 1)
    STR             r3 , [r1, #108]     @y[26+1] = sum_1qi + (sum_3qr << 1)

    @Third Butterfly
    LDR             r2, [r0, #64]       @mul_0qr = inp_0qr = x[16]

    LDR             r5, [r0, #72]       @inp_1qr = x[18]
    LDR             r6, [r0, #76]       @inp_1qi = x[19]

    @Moved for delay slot
    LDR             r3, [r0, #68]       @mul_0qi = inp_1qr = x[17]

    ADD             r4, r5, r6          @(inp_1qr + inp_1qi)
    SMULWB          r4, r4, r14         @mul_1qr = mpy_16_32_ns(0x5A83 , (inp_1qr + inp_1qi))
    SUB             r5, r6, r5          @(-inp_1qr + inp_1qi)
    SMULWB          r5, r5, r14         @mul_1qi = mpy_16_32_ns(0x5A83 , (-inp_1qr + inp_1qi))

    LDR             r6, [r0, #84]       @mul_2qr = inp_2qi = x[21]

    LDR             r9 , [r0, #88]      @inp_3qr = x[22]
    LDR             r10, [r0, #92]      @inp_3qi = x[23]

    @Moved for delay slot
    LDR             r7, [r0, #80]       @mul_2qi = inp_2qr = x[20]

    SUB             r8 , r10, r9        @(-inp_3qr + inp_3qi)
    SMULWB          r8 , r8 , r14       @mul_3qr = mpy_16_32_ns( 0x5A83 , (-inp_3qr + inp_3qi))
    ADD             r9 , r9 , r10       @(inp_3qr + inp_3qi)
    SMULWT          r9 , r9 , r14       @mul_3qi = mpy_16_32_ns(-0x5A83 , (inp_3qr + inp_3qi))

    ADD             r10, r2, r6         @sum_0qr  = mul_0qr + mul_2qr
    SUB             r2 , r2, r6         @sum_1qr  = mul_0qr - mul_2qr
    ADD             r6 , r4, r8         @sum_2qr  = mul_1qr + mul_3qr
    SUB             r4 , r4, r8         @sum_3qr  = mul_1qr - mul_3qr

    SUB             r8 , r3, r7         @sum_0qi  = mul_0qi - mul_2qi
    ADD             r3 , r3, r7         @sum_1qi  = mul_0qi + mul_2qi
    ADD             r7 , r5, r9         @sum_2qi  = mul_1qi + mul_3qi
    SUB             r5 , r5, r9         @sum_3qi  = mul_1qi - mul_3qi

    ADD             r9 , r10, r6, lsl #1 @sum_0qr + (sum_2qr << 1)
    SUB             r10, r10, r6, lsl #1 @sum_0qr - (sum_2qr << 1)
    ADD             r6 , r2 , r5, lsl #1 @sum_1qr + (sum_3qi << 1)
    SUB             r2 , r2 , r5, lsl #1 @sum_1qr - (sum_3qi << 1)
    STR             r9 , [r1, #16]      @y[4 ] = sum_0qr + (sum_2qr << 1)
    STR             r10, [r1, #80]      @y[20] = sum_0qr - (sum_2qr << 1)
    STR             r6 , [r1, #48]      @y[12] = sum_1qr + (sum_3qi << 1)
    STR             r2 , [r1, #112]     @y[28] = sum_1qr - (sum_3qi << 1)

    ADD             r5, r8, r7, lsl #1  @sum_0qi + (sum_2qi << 1)
    SUB             r8, r8, r7, lsl #1  @sum_0qi - (sum_2qi << 1)
    SUB             r7, r3, r4, lsl #1  @sum_1qi - (sum_3qr << 1)
    ADD             r3, r3, r4, lsl #1  @sum_1qi + (sum_3qr << 1)
    STR             r5 , [r1, #20]      @y[4 +1] = sum_0qi + (sum_2qi << 1)
    STR             r8 , [r1, #84]      @y[20+1] = sum_0qi - (sum_2qi << 1)
    STR             r7 , [r1, #52]      @y[12+1] = sum_1qi - (sum_3qr << 1)
    STR             r3 , [r1, #116]     @y[28+1] = sum_1qi + (sum_3qr << 1)

    @Fourth Butterfly
    LDR             r2, [r0, #96]       @mul_0qr = inp_0qr = x[24]
    LDR             r3, [r0, #100]      @mul_0qi = inp_1qr = x[25]

    LDR             r5, [r0, #104]      @inp_1qr = x[26]
    LDR             r6, [r0, #108]      @inp_1qi = x[27]
    SMULWB          r4, r5, r12         @mul_1qr = mpy_16_32_ns( 0x30FC , inp_1qr)
    SMLAWB          r4, r6, r11, r4     @mul_1qr -= mpy_16_32_ns(-0x7642 , inp_1qi)
    SMULWT          r5, r5, r11         @mul_1qi = mpy_16_32_ns(-0x7642 , inp_1qr)

    LDR             r7, [r0, #112]      @inp_2qr = x[28]
    LDR             r8, [r0, #116]      @inp_2qi = x[29]

    @Moved for delay slot
    SMLAWB          r5, r6, r12, r5     @mul_1qi += mpy_16_32_ns( 0x30FC , inp_1qi)

    SUB             r6, r8, r7          @(-inp_2qr + inp_2qi)
    SMULWB          r6, r6, r14         @mul_2qr = mpy_16_32_ns( 0x5A83 , (-inp_2qr + inp_2qi))
    ADD             r7, r8, r7          @(inp_2qr + inp_2qi)
    SMULWT          r7, r7, r14         @mul_2qi = mpy_16_32_ns(-0x5A83 , (inp_2qr + inp_2qi))

    LDR             r9 , [r0, #120]     @inp_3qr = x[30]
    LDR             r10, [r0, #124]     @inp_3qi = x[31]
    SMULWT          r8, r9 , r11        @mul_3qr = mpy_16_32_ns(-0x7642 , inp_3qr)
    SMLAWT          r8, r10, r12, r8    @mul_3qr -= mpy_16_32_ns( 0x30FC , inp_3qi)@
    SMULWB          r9, r9 , r12        @mul_3qi = mpy_16_32_ns( 0x30FC , inp_3qr)
    SMLAWT          r9, r10, r11, r9    @mul_3qi += mpy_16_32_ns(-0x7642 , inp_3qi)

    ADD             r10, r2, r6, lsl #1 @sum_0qr  = mul_0qr + (mul_2qr << 1)
    SUB             r2 , r2, r6, lsl #1 @sum_1qr  = mul_0qr - (mul_2qr << 1)
    ADD             r6 , r4, r8         @sum_2qr  = mul_1qr + mul_3qr
    SUB             r4 , r4, r8         @sum_3qr  = mul_1qr - mul_3qr

    ADD             r8 , r3, r7, lsl #1 @sum_0qi  = mul_0qi + (mul_2qi << 1)
    SUB             r3 , r3, r7, lsl #1 @sum_1qi  = mul_0qi - (mul_2qi << 1)
    ADD             r7 , r5, r9         @sum_2qi  = mul_1qi + mul_3qi
    SUB             r5 , r5, r9         @sum_3qi  = mul_1qi - mul_3qi

    ADD             r9 , r10, r6, lsl #1 @sum_0qr + (sum_2qr << 1)
    SUB             r10, r10, r6, lsl #1 @sum_0qr - (sum_2qr << 1)
    ADD             r6 , r2 , r5, lsl #1 @sum_1qr + (sum_3qi << 1)
    SUB             r2 , r2 , r5, lsl #1 @sum_1qr - (sum_3qi << 1)
    STR             r9 , [r1, #24]      @y[6 ] = sum_0qr + (sum_2qr << 1)
    STR             r10, [r1, #88]      @y[22] = sum_0qr - (sum_2qr << 1)
    STR             r6 , [r1, #56]      @y[14] = sum_1qr + (sum_3qi << 1)
    STR             r2 , [r1, #120]     @y[30] = sum_1qr - (sum_3qi << 1)

    ADD             r5 , r8 , r7, lsl #1 @sum_0qi + (sum_2qi << 1)
    SUB             r8 , r8 , r7, lsl #1 @sum_0qi - (sum_2qi << 1)
    SUB             r7 , r3 , r4, lsl #1 @sum_1qi - (sum_3qr << 1)
    ADD             r3 , r3 , r4, lsl #1 @sum_1qi + (sum_3qr << 1)
    STR             r5 , [r1, #28]      @y[6 +1] = sum_0qi + (sum_2qi << 1)
    STR             r8 , [r1, #92]      @y[22+1] = sum_0qi - (sum_2qi << 1)
    STR             r7 , [r1, #60]      @y[14+1] = sum_1qi - (sum_3qr << 1)
    STR             r3 , [r1, #124]     @y[30+1] = sum_1qi + (sum_3qr << 1)

    LDMFD           sp!, {r4-r12, r15}

