.macro push_v_regs
    stp             X8, X9, [sp, #-16]!
    stp             X10, X11, [sp, #-16]!
    stp             X12, X13, [sp, #-16]!
    stp             X14, X15, [sp, #-16]!
    stp             X16, X17, [sp, #-16]!
    stp             X18, X19, [sp, #-16]!
    stp             X20, X21, [sp, #-16]!
    stp             X22, X24, [sp, #-16]!
    stp             X29, X30, [sp, #-16]!
.endm

.macro pop_v_regs
    ldp             X29, X30, [sp], #16
    ldp             X22, X24, [sp], #16
    ldp             X20, X21, [sp], #16
    ldp             X18, X19, [sp], #16
    ldp             X16, X17, [sp], #16
    ldp             X14, X15, [sp], #16
    ldp             X12, X13, [sp], #16
    ldp             X10, X11, [sp], #16
    ldp             X8, X9, [sp], #16
.endm


.text
.p2align 2
.global ixheaacd_fft32x32_ld2_armv8

ixheaacd_fft32x32_ld2_armv8:

    // STMFD sp!, {x4-x12,x14}
    push_v_regs
    stp             x19, x20, [sp, #-16]!

    //DIT Radix-4 FFT First Stage
    //First Butterfly
    MOV             x0, x2
    MOV             x1, x3
    LDR             w2, [x0]            //x_0 = x[0 ]
    sxtw            x2, w2
    LDR             w3, [x0, #32]       //x_2 = x[8 ]
    sxtw            x3, w3
    LDR             w4, [x0, #64]       //x_4 = x[16]
    sxtw            x4, w4
    LDR             w5, [x0, #96]       //x_6 = x[24]
    sxtw            x5, w5
    ADD             w6, w2, w4          //xh0_0 = x_0 + x_4
    SUB             w7, w2, w4          //xl0_0 = x_0 - x_4
    ADD             w8, w3, w5          //xh0_1 = x_2 + x_6
    SUB             w9, w3, w5          //xl0_1 = x_2 - x_6

    LDR             w2, [x0, #4]        //x_1 = x[0 +1]
    sxtw            x2, w2
    LDR             w3, [x0, #36]       //x_3 = x[8 +1]
    sxtw            x3, w3
    LDR             w4, [x0, #68]       //x_5 = x[16+1]
    sxtw            x4, w4
    LDR             w5, [x0, #100]      //x_7 = x[24+1]
    sxtw            x5, w5
    ADD             w10, w2, w4         //xh1_0 = x_1 + x_5
    SUB             w11, w2, w4         //xl1_0 = x_1 - x_5
    ADD             w12, w3, w5         //xh1_1 = x_3 + x_7
    SUB             w14, w3, w5         //xl1_1 = x_3 - x_7

    ADD             w2, w6, w8          //n00 = xh0_0 + xh0_1
    ADD             w3, w7, w14         //n10 = xl0_0 + xl1_1
    SUB             w4, w6, w8          //n20 = xh0_0 - xh0_1
    SUB             w5, w7, w14         //n30 = xl0_0 - xl1_1
    STR             w2, [x0]            //x[0 ] = n00
    STR             w3, [x0, #32]       //x[8 ] = n10
    STR             w4, [x0, #64]       //x[16] = n20
    STR             w5, [x0, #96]       //x[24] = n30

    ADD             w2, w10, w12        //n01 = xh1_0 + xh1_1
    SUB             w3, w11, w9         //n11 = xl1_0 - xl0_1
    SUB             w4, w10, w12        //n21 = xh1_0 - xh1_1
    ADD             w5, w11, w9         //n31 = xl1_0 + xl0_1
    STR             w2, [x0, #4]        //x[1   ] = n01
    STR             w3, [x0, #36]       //x[8+1 ] = n11
    STR             w4, [x0, #68]       //x[16+1] = n21
    STR             w5, [x0, #100]      //x[24+1] = n31

    //Second Butterfly
    LDR             w2, [x0, #8]        //x_0 = x[2 ]
    sxtw            x2, w2
    LDR             w3, [x0, #40]       //x_2 = x[10]
    sxtw            x3, w3
    LDR             w4, [x0, #72]       //x_4 = x[18]
    sxtw            x4, w4
    LDR             w5, [x0, #104]      //x_6 = x[26]
    sxtw            x5, w5
    ADD             w6, w2, w4          //xh0_0 = x_0 + x_4
    SUB             w7, w2, w4          //xl0_0 = x_0 - x_4
    ADD             w8, w3, w5          //xh0_1 = x_2 + x_6
    SUB             w9, w3, w5          //xl0_1 = x_2 - x_6

    LDR             w2, [x0, #12]       //x_1 = x[2 +1]
    sxtw            x2, w2
    LDR             w3, [x0, #44]       //x_3 = x[10+1]
    sxtw            x3, w3
    LDR             w4, [x0, #76]       //x_5 = x[18+1]
    sxtw            x4, w4
    LDR             w5, [x0, #108]      //x_7 = x[26+1]
    sxtw            x5, w5
    ADD             w10, w2, w4         //xh1_0 = x_1 + x_5
    SUB             w11, w2, w4         //xl1_0 = x_1 - x_5
    ADD             w12, w3, w5         //xh1_1 = x_3 + x_7
    SUB             w14, w3, w5         //xl1_1 = x_3 - x_7

    ADD             w2, w6, w8          //n00 = xh0_0 + xh0_1
    ADD             w3, w7, w14         //n10 = xl0_0 + xl1_1
    SUB             w4, w6, w8          //n20 = xh0_0 - xh0_1
    SUB             w5, w7, w14         //n30 = xl0_0 - xl1_1
    STR             w2, [x0, #8]        //x[2 ] = n00
    STR             w3, [x0, #40]       //x[10] = n10
    STR             w4, [x0, #72]       //x[18] = n20
    STR             w5, [x0, #104]      //x[26] = n30

    ADD             w2, w10, w12        //n01 = xh1_0 + xh1_1
    SUB             w3, w11, w9         //n11 = xl1_0 - xl0_1
    SUB             w4, w10, w12        //n21 = xh1_0 - xh1_1
    ADD             w5, w11, w9         //n31 = xl1_0 + xl0_1
    STR             w2, [x0, #12]       //x[2 +1] = n01
    STR             w3, [x0, #44]       //x[10+1] = n11
    STR             w4, [x0, #76]       //x[18+1] = n21
    STR             w5, [x0, #108]      //x[26+1] = n31

    //Third Butterfly
    LDR             w2, [x0, #16]       //x_0 = x[4 ]
    sxtw            x2, w2
    LDR             w3, [x0, #48]       //x_2 = x[12]
    sxtw            x3, w3
    LDR             w4, [x0, #80]       //x_4 = x[20]
    sxtw            x4, w4
    LDR             w5, [x0, #112]      //x_6 = x[28]
    sxtw            x5, w5
    ADD             w6, w2, w4          //xh0_0 = x_0 + x_4
    SUB             w7, w2, w4          //xl0_0 = x_0 - x_4
    ADD             w8, w3, w5          //xh0_1 = x_2 + x_6
    SUB             w9, w3, w5          //xl0_1 = x_2 - x_6

    LDR             w2, [x0, #20]       //x_1 = x[4 +1]
    sxtw            x2, w2
    LDR             w3, [x0, #52]       //x_3 = x[12+1]
    sxtw            x3, w3
    LDR             w4, [x0, #84]       //x_5 = x[20+1]
    sxtw            x4, w4
    LDR             w5, [x0, #116]      //x_7 = x[28+1]
    sxtw            x5, w5
    ADD             w10, w2, w4         //xh1_0 = x_1 + x_5
    SUB             w11, w2, w4         //xl1_0 = x_1 - x_5
    ADD             w12, w3, w5         //xh1_1 = x_3 + x_7
    SUB             w14, w3, w5         //xl1_1 = x_3 - x_7

    ADD             w2, w6, w8          //n00 = xh0_0 + xh0_1
    ADD             w3, w7, w14         //n10 = xl0_0 + xl1_1
    SUB             w4, w6, w8          //n20 = xh0_0 - xh0_1
    SUB             w5, w7, w14         //n30 = xl0_0 - xl1_1
    STR             w2, [x0, #16]       //x[4 ] = n00
    STR             w3, [x0, #48]       //x[12] = n10
    STR             w4, [x0, #80]       //x[20] = n20
    STR             w5, [x0, #112]      //x[28] = n30

    ADD             w2, w10, w12        //n01 = xh1_0 + xh1_1
    SUB             w3, w11, w9         //n11 = xl1_0 - xl0_1
    SUB             w4, w10, w12        //n21 = xh1_0 - xh1_1
    ADD             w5, w11, w9         //n31 = xl1_0 + xl0_1
    STR             w2, [x0, #20]       //x[4 +1] = n01
    STR             w3, [x0, #52]       //x[12+1] = n11
    STR             w4, [x0, #84]       //x[20+1] = n21
    STR             w5, [x0, #116]      //x[28+1] = n31

    //Fourth Butterfly
    LDR             w2, [x0, #24]       //x_0 = x[6 ]
    sxtw            x2, w2
    LDR             w3, [x0, #56]       //x_2 = x[14]
    sxtw            x3, w3
    LDR             w4, [x0, #88]       //x_4 = x[22]
    sxtw            x4, w4
    LDR             w5, [x0, #120]      //x_6 = x[30]
    sxtw            x5, w5
    ADD             w6, w2, w4          //xh0_0 = x_0 + x_4
    SUB             w7, w2, w4          //xl0_0 = x_0 - x_4
    ADD             w8, w3, w5          //xh0_1 = x_2 + x_6
    SUB             w9, w3, w5          //xl0_1 = x_2 - x_6

    LDR             w2, [x0, #28]       //x_1 = x[6 +1]
    sxtw            x2, w2
    LDR             w3, [x0, #60]       //x_3 = x[14+1]
    sxtw            x3, w3
    LDR             w4, [x0, #92]       //x_5 = x[22+1]
    sxtw            x4, w4
    LDR             w5, [x0, #124]      //x_7 = x[30+1]
    sxtw            x5, w5
    ADD             w10, w2, w4         //xh1_0 = x_1 + x_5
    SUB             w11, w2, w4         //xl1_0 = x_1 - x_5
    ADD             w12, w3, w5         //xh1_1 = x_3 + x_7
    SUB             w14, w3, w5         //xl1_1 = x_3 - x_7

    ADD             w2, w6, w8          //n00 = xh0_0 + xh0_1
    ADD             w3, w7, w14         //n10 = xl0_0 + xl1_1
    SUB             w4, w6, w8          //n20 = xh0_0 - xh0_1
    SUB             w5, w7, w14         //n30 = xl0_0 - xl1_1
    STR             w2, [x0, #24]       //x[6 ] = n00
    STR             w3, [x0, #56]       //x[14] = n10
    STR             w4, [x0, #88]       //x[22] = n20
    STR             w5, [x0, #120]      //x[30] = n30

    ADD             w2, w10, w12        //n01 = xh1_0 + xh1_1
    SUB             w3, w11, w9         //n11 = xl1_0 - xl0_1
    SUB             w4, w10, w12        //n21 = xh1_0 - xh1_1
    ADD             w5, w11, w9         //n31 = xl1_0 + xl0_1
    STR             w2, [x0, #28]       //x[6 +1] = n01
    STR             w3, [x0, #60]       //x[14+1] = n11
    STR             w4, [x0, #92]       //x[22+1] = n21
    STR             w5, [x0, #124]      //x[30+1] = n31


    //DIT Radix-4 FFT Second Stage
    //First Butterfly
    LDR             w2, [x0]            //inp_0qr = x[0]
    sxtw            x2, w2
    LDR             w3, [x0, #8]        //inp_1qr = x[2]
    sxtw            x3, w3
    LDR             w4, [x0, #16]       //inp_2qr = x[4]
    sxtw            x4, w4
    LDR             w5, [x0, #24]       //inp_3qr = x[6]
    sxtw            x5, w5
    ADD             w6, w2, w4          //sum_0qr  = mul_0qr + mul_2qr
    SUB             w7, w2, w4          //sum_1qr  = mul_0qr - mul_2qr
    ADD             w8, w3, w5          //sum_2qr  = mul_1qr + mul_3qr
    SUB             w9, w3, w5          //sum_3qr  = mul_1qr - mul_3qr

    LDR             w2, [x0, #4]        //inp_0qi = x[1]
    sxtw            x2, w2
    LDR             w3, [x0, #12]       //inp_1qi = x[3]
    sxtw            x3, w3
    LDR             w4, [x0, #20]       //inp_2qi = x[5]
    sxtw            x4, w4
    LDR             w5, [x0, #28]       //inp_3qi = x[7]
    sxtw            x5, w5
    ADD             w10, w2, w4         //sum_0qi  = mul_0qi + mul_2qi
    SUB             w11, w2, w4         //sum_1qi  = mul_0qi - mul_2qi
    ADD             w12, w3, w5         //sum_2qi  = mul_1qi + mul_3qi
    SUB             w14, w3, w5         //sum_3qi  = mul_1qi - mul_3qi

    ADD             w2, w6, w8          //sum_0qr + sum_2qr
    ADD             w3, w7, w14         //sum_1qr + sum_3qi
    SUB             w4, w6, w8          //sum_0qr - sum_2qr
    SUB             w5, w7, w14         //sum_1qr - sum_3qi
    STR             w2, [x1]            //y[0 ] = sum_0qr + sum_2qr
    STR             w3, [x1, #32]       //y[8 ] = sum_1qr + sum_3qi
    STR             w4, [x1, #64]       //y[16] = sum_0qr - sum_2qr
    STR             w5, [x1, #96]       //y[24] = sum_1qr - sum_3qi

    ADD             w2, w10, w12        //sum_0qi + sum_2qi
    SUB             w3, w11, w9         //sum_1qi - sum_3qr
    SUB             w4, w10, w12        //sum_0qi - sum_2qi
    ADD             w5, w11, w9         //sum_1qi + sum_3qr
    STR             w2, [x1, #4]        //y[0 +1] = sum_0qi + sum_2qi
    STR             w3, [x1, #36]       //y[8 +1] = sum_1qi - sum_3qr
    STR             w4, [x1, #68]       //y[16+1] = sum_0qi - sum_2qi
    STR             w5, [x1, #100]      //y[24+1] = sum_1qi + sum_3qr


    //Load twiddle factors
//    LDR w11,  =2310960706            //0x89BE7642
    MOV             w11, #0x7642
    sxth            w11, w11
    MOV             w21, #0x89BE
    sxth            w21, w21
//    LDR w12,  =3473158396            //0xCF0430FC
    MOV             w12, #0x30FC
    sxth            w12, w12
    MOV             w22, #0xCF04
    sxth            w22, w22
//    LDR w14,  =2776455811            //0xA57D5A83
    MOV             w14, #0x5A83
    sxth            w14, w14
    MOV             w24, #0xA57D
    sxth            w24, w24

    //Second Butterfly
    LDR             w2, [x0, #32]       //mul_0qr = inp_0qr = x[8]
    sxtw            x2, w2
    LDR             w3, [x0, #36]       //mul_0qi = inp_1qr = x[9]
    sxtw            x3, w3

    LDR             w5, [x0, #40]       //inp_1qr = x[10]
    sxtw            x5, w5
    LDR             w6, [x0, #44]       //inp_1qi = x[11]
    sxtw            x6, w6

    SMULL           x4, w5, w11
    ASR             x4, x4, #16
//    SMULWB      x4, x5, x11                 //mul_1qr = mpy_16_32_ns( 0x7642 , inp_1qr)

    SMULL           x20, w6, w12
    ASR             x20, x20, #16
    ADD             w4, w4, w20
//    SMLAWB      x4, x6, x12, x4             //mul_1qr -= mpy_16_32_ns(-0x30FC , inp_1qi)

    SMULL           x5, w5, w22
    ASR             x5, x5, #16
//  SMULWT      x5, x5, x12                 //mul_1qi = mpy_16_32_ns(-0x30FC , inp_1qr)

    LDR             w7, [x0, #48]       //inp_2qr = x[12]
    sxtw            x7, w7
    LDR             w8, [x0, #52]       //inp_2qi = x[13]
    sxtw            x8, w8

    //Moved for delay slot
    SMULL           x20, w6, w11
    ASR             x20, x20, #16
    ADD             w5, w5, w20
//    SMLAWB      x5, x6, x11, x5             //mul_1qi += mpy_16_32_ns( 0x7642 , inp_1qi)

    ADD             w6, w7, w8          //(inp_2qr + inp_2qi)

    SMULL           x6, w6, w14
    ASR             x6, x6, #16
//    SMULWB      x6, x6, x14                 //mul_2qr = mpy_16_32_ns(0x5A83 , (inp_2qr + inp_2qi))

    SUB             w7, w8, w7          //(-inp_2qr + inp_2qi)

    SMULL           x7, w7, w14
    ASR             x7, x7, #16
//  SMULWB      x7, x7, x14                 //mul_2qi = mpy_16_32_ns(0x5A83 , (-inp_2qr + inp_2qi))

    LDR             x9 , [x0, #56]      //inp_3qr = x[14]
    sxtw            x9, w9
    LDR             w10, [x0, #60]      //inp_3qi = x[15]
    sxtw            x10, w10

    SMULL           x8, w9, w12
    ASR             x8, x8, #16
//    SMULWB      x8, x9 , x12                //mul_3qr = mpy_16_32_ns( 0x30FC , inp_3qr)

    SMULL           x20, w10, w11
    ASR             x20, x20, #16
    ADD             w8, w8, w20
//  SMLAWB      x8, x10, x11, x8            //mul_3qr -= mpy_16_32_ns(-0x7642 , inp_3qi)//

    SMULL           x9, w9 , w21
    ASR             x9, x9, #16
//  SMULWT      x9, x9 , x11                //mul_3qi = mpy_16_32_ns(-0x7642 , inp_3qr)

    SMULL           x20, w10, w12
    ASR             x20, x20, #16
    ADD             w9, w9, w20
//    SMLAWB      x9, x10, x12, x9            //mul_3qi += mpy_16_32_ns( 0x30FC , inp_3qi)

    ADD             w10, w2, w6, lsl #1 //sum_0qr  = mul_0qr + (mul_2qr << 1)
    SUB             w2 , w2, w6, lsl #1 //sum_1qr  = mul_0qr - (mul_2qr << 1)
    ADD             w6 , w4, w8         //sum_2qr  = mul_1qr + mul_3qr
    SUB             w4 , w4, w8         //sum_3qr  = mul_1qr - mul_3qr

    ADD             w8 , w3, w7, lsl #1 //sum_0qi  = mul_0qi + (mul_2qi << 1)
    SUB             w3 , w3, w7, lsl #1 //sum_1qi  = mul_0qi - (mul_2qi << 1)
    ADD             w7 , w5, w9         //sum_2qi  = mul_1qi + mul_3qi
    SUB             w5 , w5, w9         //sum_3qi  = mul_1qi - mul_3qi

    ADD             w9 , w10, w6, lsl #1 //sum_0qr + (sum_2qr << 1)
    SUB             w10, w10, w6, lsl #1 //sum_0qr - (sum_2qr << 1)
    ADD             w6 , w2 , w5, lsl #1 //sum_1qr + (sum_3qi << 1)
    SUB             w2 , w2 , w5, lsl #1 //sum_1qr - (sum_3qi << 1)
    STR             w9 , [x1, #8]       //y[2 ] = sum_0qr + (sum_2qr << 1)
    STR             w10, [x1, #72]      //y[18] = sum_0qr - (sum_2qr << 1)
    STR             w6 , [x1, #40]      //y[10] = sum_1qr + (sum_3qi << 1)
    STR             w2 , [x1, #104]     //y[26] = sum_1qr - (sum_3qi << 1)

    ADD             w5 , w8 , w7, lsl #1 //sum_0qi + (sum_2qi << 1)
    SUB             w8 , w8 , w7, lsl #1 //sum_0qi - (sum_2qi << 1)
    SUB             w7 , w3 , w4, lsl #1 //sum_1qi - (sum_3qr << 1)
    ADD             w3 , w3 , w4, lsl #1 //sum_1qi + (sum_3qr << 1)
    STR             w5 , [x1, #12]      //y[2 +1] = sum_0qi + (sum_2qi << 1)
    STR             w8 , [x1, #76]      //y[18+1] = sum_0qi - (sum_2qi << 1)
    STR             w7 , [x1, #44]      //y[10+1] = sum_1qi - (sum_3qr << 1)
    STR             w3 , [x1, #108]     //y[26+1] = sum_1qi + (sum_3qr << 1)

    //Third Butterfly
    LDR             w2, [x0, #64]       //mul_0qr = inp_0qr = x[16]
    sxtw            x2, w2
    LDR             w5, [x0, #72]       //inp_1qr = x[18]
    sxtw            x5, w5
    LDR             w6, [x0, #76]       //inp_1qi = x[19]
    sxtw            x6, w6
    //Moved for delay slot
    LDR             w3, [x0, #68]       //mul_0qi = inp_1qr = x[17]
    sxtw            x3, w3

    ADD             w4, w5, w6          //(inp_1qr + inp_1qi)

    SMULL           x4, w4, w14
    ASR             x4, x4, #16
//    SMULWB      x4, x4, x14                 //mul_1qr = mpy_16_32_ns(0x5A83 , (inp_1qr + inp_1qi))
    SUB             w5, w6, w5          //(-inp_1qr + inp_1qi)

    SMULL           x5, w5, w14
    ASR             x5, x5, #16
//  SMULWB      x5, x5, x14                 //mul_1qi = mpy_16_32_ns(0x5A83 , (-inp_1qr + inp_1qi))

    LDR             w6, [x0, #84]       //mul_2qr = inp_2qi = x[21]
    sxtw            x6, w6
    LDR             x9 , [x0, #88]      //inp_3qr = x[22]
    sxtw            x9, w9
    LDR             w10, [x0, #92]      //inp_3qi = x[23]
    sxtw            x10, w10
    //Moved for delay slot
    LDR             w7, [x0, #80]       //mul_2qi = inp_2qr = x[20]
    sxtw            x7, w7

    SUB             w8 , w10, w9        //(-inp_3qr + inp_3qi)

    SMULL           x8, w8, w14
    ASR             x8, x8, #16
//    SMULWB      x8 , x8 , x14               //mul_3qr = mpy_16_32_ns( 0x5A83 , (-inp_3qr + inp_3qi))

    ADD             w9 , w9 , w10       //(inp_3qr + inp_3qi)

    SMULL           x9, w9, w24
    ASR             x9, x9, #16
//    SMULWT      x9 , x9 , x14               //mul_3qi = mpy_16_32_ns(-0x5A83 , (inp_3qr + inp_3qi))

    ADD             w10, w2, w6         //sum_0qr  = mul_0qr + mul_2qr
    SUB             w2 , w2, w6         //sum_1qr  = mul_0qr - mul_2qr
    ADD             w6 , w4, w8         //sum_2qr  = mul_1qr + mul_3qr
    SUB             w4 , w4, w8         //sum_3qr  = mul_1qr - mul_3qr

    SUB             w8 , w3, w7         //sum_0qi  = mul_0qi - mul_2qi
    ADD             w3 , w3, w7         //sum_1qi  = mul_0qi + mul_2qi
    ADD             w7 , w5, w9         //sum_2qi  = mul_1qi + mul_3qi
    SUB             w5 , w5, w9         //sum_3qi  = mul_1qi - mul_3qi

    ADD             w9 , w10, w6, lsl #1 //sum_0qr + (sum_2qr << 1)
    SUB             w10, w10, w6, lsl #1 //sum_0qr - (sum_2qr << 1)
    ADD             w6 , w2 , w5, lsl #1 //sum_1qr + (sum_3qi << 1)
    SUB             w2 , w2 , w5, lsl #1 //sum_1qr - (sum_3qi << 1)
    STR             w9 , [x1, #16]      //y[4 ] = sum_0qr + (sum_2qr << 1)
    STR             w10, [x1, #80]      //y[20] = sum_0qr - (sum_2qr << 1)
    STR             w6 , [x1, #48]      //y[12] = sum_1qr + (sum_3qi << 1)
    STR             w2 , [x1, #112]     //y[28] = sum_1qr - (sum_3qi << 1)

    ADD             w5, w8, w7, lsl #1  //sum_0qi + (sum_2qi << 1)
    SUB             w8, w8, w7, lsl #1  //sum_0qi - (sum_2qi << 1)
    SUB             w7, w3, w4, lsl #1  //sum_1qi - (sum_3qr << 1)
    ADD             w3, w3, w4, lsl #1  //sum_1qi + (sum_3qr << 1)
    STR             w5 , [x1, #20]      //y[4 +1] = sum_0qi + (sum_2qi << 1)
    STR             w8 , [x1, #84]      //y[20+1] = sum_0qi - (sum_2qi << 1)
    STR             w7 , [x1, #52]      //y[12+1] = sum_1qi - (sum_3qr << 1)
    STR             w3 , [x1, #116]     //y[28+1] = sum_1qi + (sum_3qr << 1)

    //Fourth Butterfly
    LDR             w2, [x0, #96]       //mul_0qr = inp_0qr = x[24]
    sxtw            x2, w2
    LDR             w3, [x0, #100]      //mul_0qi = inp_1qr = x[25]
    sxtw            x3, w3

    LDR             w5, [x0, #104]      //inp_1qr = x[26]
    sxtw            x5, w5
    LDR             w6, [x0, #108]      //inp_1qi = x[27]
    sxtw            x6, w6

    SMULL           x4, w5, w12
    ASR             x4, x4, #16
//    SMULWB      x4, x5, x12                 //mul_1qr = mpy_16_32_ns( 0x30FC , inp_1qr)

    SMULL           x20, w6, w11
    ASR             x20, x20, #16
    ADD             w4, w4, w20
//  SMLAWB      x4, x6, x11, x4             //mul_1qr -= mpy_16_32_ns(-0x7642 , inp_1qi)

    SMULL           x5, w5, w21
    ASR             x5, x5, #16
//  SMULWT      x5, x5, x11                 //mul_1qi = mpy_16_32_ns(-0x7642 , inp_1qr)

    LDR             w7, [x0, #112]      //inp_2qr = x[28]
    sxtw            x7, w7
    LDR             w8, [x0, #116]      //inp_2qi = x[29]
    sxtw            x8, w8

    //Moved for delay slot
    SMULL           x20, w6, w12
    ASR             x20, x20, #16
    ADD             w5, w5, w20
//    SMLAWB      x5, x6, x12, x5             //mul_1qi += mpy_16_32_ns( 0x30FC , inp_1qi)

    SUB             w6, w8, w7          //(-inp_2qr + inp_2qi)

    SMULL           x6, w6, w14
    ASR             x6, x6, #16
//    SMULWB      x6, x6, x14                 //mul_2qr = mpy_16_32_ns( 0x5A83 , (-inp_2qr + inp_2qi))
    ADD             w7, w8, w7          //(inp_2qr + inp_2qi)

    SMULL           x7, w7, w24
    ASR             x7, x7, #16
//   SMULWT      x7, x7, x14                 //mul_2qi = mpy_16_32_ns(-0x5A83 , (inp_2qr + inp_2qi))

    LDR             w9 , [x0, #120]     //inp_3qr = x[30]
    sxtw            x9, w9
    LDR             w10, [x0, #124]     //inp_3qi = x[31]
    sxtw            x10, w10

    SMULL           x8, w9, w21
    ASR             x8, x8, #16
//    SMULWT      x8, x9 , x11                //mul_3qr = mpy_16_32_ns(-0x7642 , inp_3qr)

    SMULL           x20, w10, w22
    ASR             x20, x20, #16
    ADD             w8, w8, w20
//  SMLAWT      x8, x10, x12, x8            //mul_3qr -= mpy_16_32_ns( 0x30FC , inp_3qi)//

    SMULL           x9, w9, w12
    ASR             x9, x9, #16
//  SMULWB      x9, x9 , x12                //mul_3qi = mpy_16_32_ns( 0x30FC , inp_3qr)

    SMULL           x20, w10, w21
    ASR             x20, x20, #16
    ADD             w9, w9, w20
//  SMLAWT      x9, x10, x11, x9            //mul_3qi += mpy_16_32_ns(-0x7642 , inp_3qi)

    ADD             w10, w2, w6, lsl #1 //sum_0qr  = mul_0qr + (mul_2qr << 1)
    SUB             w2 , w2, w6, lsl #1 //sum_1qr  = mul_0qr - (mul_2qr << 1)
    ADD             w6 , w4, w8         //sum_2qr  = mul_1qr + mul_3qr
    SUB             w4 , w4, w8         //sum_3qr  = mul_1qr - mul_3qr

    ADD             w8 , w3, w7, lsl #1 //sum_0qi  = mul_0qi + (mul_2qi << 1)
    SUB             w3 , w3, w7, lsl #1 //sum_1qi  = mul_0qi - (mul_2qi << 1)
    ADD             w7 , w5, w9         //sum_2qi  = mul_1qi + mul_3qi
    SUB             w5 , w5, w9         //sum_3qi  = mul_1qi - mul_3qi

    ADD             w9 , w10, w6, lsl #1 //sum_0qr + (sum_2qr << 1)
    SUB             w10, w10, w6, lsl #1 //sum_0qr - (sum_2qr << 1)
    ADD             w6 , w2 , w5, lsl #1 //sum_1qr + (sum_3qi << 1)
    SUB             w2 , w2 , w5, lsl #1 //sum_1qr - (sum_3qi << 1)
    STR             w9 , [x1, #24]      //y[6 ] = sum_0qr + (sum_2qr << 1)
    STR             w10, [x1, #88]      //y[22] = sum_0qr - (sum_2qr << 1)
    STR             w6 , [x1, #56]      //y[14] = sum_1qr + (sum_3qi << 1)
    STR             w2 , [x1, #120]     //y[30] = sum_1qr - (sum_3qi << 1)

    ADD             w5 , w8 , w7, lsl #1 //sum_0qi + (sum_2qi << 1)
    SUB             w8 , w8 , w7, lsl #1 //sum_0qi - (sum_2qi << 1)
    SUB             w7 , w3 , w4, lsl #1 //sum_1qi - (sum_3qr << 1)
    ADD             w3 , w3 , w4, lsl #1 //sum_1qi + (sum_3qr << 1)
    STR             w5 , [x1, #28]      //y[6 +1] = sum_0qi + (sum_2qi << 1)
    STR             w8 , [x1, #92]      //y[22+1] = sum_0qi - (sum_2qi << 1)
    STR             w7 , [x1, #60]      //y[14+1] = sum_1qi - (sum_3qr << 1)
    STR             w3 , [x1, #124]     //y[30+1] = sum_1qi + (sum_3qr << 1)

    // LDMFD sp!, {x4-x12,x15}
    ldp             x19, x20, [sp], #16
    pop_v_regs
    ret

