//VOID ixheaacd_inv_dit_fft_8pt(WORD32 *y,
//                            WORD32 *real,
//                            WORD32 *imag)

.macro push_v_regs
    stp             q8, q9, [sp, #-32]!
    stp             q10, q11, [sp, #-32]!
    stp             q12, q13, [sp, #-32]!
    stp             q14, q15, [sp, #-32]!
.endm
.macro pop_v_regs
    ldp             q14, q15, [sp], #32
    ldp             q12, q13, [sp], #32
    ldp             q10, q11, [sp], #32
    ldp             q8, q9, [sp], #32
.endm


.text
.global ixheaacd_inv_dit_fft_8pt_armv8
ixheaacd_inv_dit_fft_8pt_armv8:
    push_v_regs
    MOV             w3, #0x5A820000
    DUP             v0.2s, w3
    MOV             x5, #8
    ADD             x6, x0, #4

    //LD2 {v1.2s,v2.2s},[x0],x5
    //LD2 {v3.2s,v4.2s},[x0],x5
    //LD2 {v5.2s,v6.2s},[x0],x5
    //LD2 {v7.2s,v8.2s},[x0],x5

    LD1             {v1.s}[0], [x0], x5
    LD1             {v2.s}[0], [x6], x5
    LD1             {v1.s}[1], [x0], x5
    LD1             {v2.s}[1], [x6], x5
    LD1             {v3.s}[0], [x0], x5
    LD1             {v4.s}[0], [x6], x5
    LD1             {v3.s}[1], [x0], x5
    LD1             {v4.s}[1], [x6], x5
    LD1             {v5.s}[0], [x0], x5
    LD1             {v6.s}[0], [x6], x5
    LD1             {v5.s}[1], [x0], x5
    LD1             {v6.s}[1], [x6], x5
    LD1             {v7.s}[0], [x0], x5
    LD1             {v8.s}[0], [x6], x5
    LD1             {v7.s}[1], [x0], x5
    LD1             {v8.s}[1], [x6], x5

    //v1 - y0_2
    //v2 - y1_3
    //v3 - y4_6
    //v4 - y5_7
    //v5 - y8_10
    //v6 - y9_11
    //v7 - y12_14
    //v8 - y13_15

    SQADD           v9.2s, v1.2s, v5.2s //a00_v = vqadd_s32(y0_2,y8_10);
    SQADD           v10.2s, v2.2s, v6.2s //a20_v = vqadd_s32(y1_3,y9_11);
    SQADD           v11.2s, v3.2s, v7.2s //a10_v = vqadd_s32(y4_6,y12_14);
    SQADD           v12.2s, v4.2s, v8.2s //a30_v = vqadd_s32(y5_7,y13_15);

    SQSUB           v1.2s, v1.2s, v5.2s //a0_v  = vqsub_s32(y0_2,y8_10);
    SQSUB           v5.2s, v2.2s, v6.2s //a3_v  = vqsub_s32(y1_3,y9_11);
    SQSUB           v2.2s, v3.2s, v7.2s //a2_v  = vqsub_s32(y4_6,y12_14);
    SQSUB           v6.2s, v4.2s, v8.2s //a1_v  = vqsub_s32(y5_7,y13_15);

    SQADD           v3.2s, v9.2s, v11.2s //x0_8  = vqadd_s32(a00_v,a10_v);
    SQADD           v7.2s, v10.2s, v12.2s //x1_9  = vqadd_s32(a20_v,a30_v);

    SQSUB           v4.2s, v9.2s, v11.2s //x4_12 = vqsub_s32(a00_v,a10_v);
    SQSUB           v8.2s, v10.2s, v12.2s //x5_13 = vqsub_s32(a20_v,a30_v);

    SQADD           v9.2s, v1.2s, v6.2s //x6_14 = vqadd_s32(a0_v,a1_v);
    SQADD           v11.2s, v5.2s, v2.2s //x3_11 = vqadd_s32(a3_v,a2_v);
    SQSUB           v10.2s, v1.2s, v6.2s //x2_10 = vqsub_s32(a0_v,a1_v);
    SQSUB           v13.2s, v5.2s, v2.2s //x7_15 = vqsub_s32(a3_v,a2_v);

    UZP1            v1.2s, v3.2s, v7.2s //x0_1 = vuzp1_s32(x0_8,x1_9);
    UZP2            v5.2s, v3.2s, v7.2s //x8_9 = vuzp2_s32(x0_8,x1_9);

    UZP1            v6.2s, v4.2s, v8.2s //x4_5      = vuzp1_s32(x4_12,x5_13);
    UZP2            v7.2s, v4.2s, v8.2s //x12_13    = vuzp2_s32(x4_12,x5_13);
    REV64           v7.2s, v7.2s        //x13_12    = vrev64_s32(x12_13);

    SQADD           v3.2s, v1.2s, v5.2s //real_imag0 = vqadd_s32(x0_1,x8_9);
    SQSUB           v8.2s, v1.2s, v5.2s //a00_10_v = vqsub_s32(x0_1,x8_9);

    SQADD           v12.2s, v6.2s, v7.2s //real_imag4 = vqadd_s32(x4_5,x13_12);
    SQSUB           v14.2s, v6.2s, v7.2s //a0_1_v    = vqsub_s32(x4_5,x13_12);


    MOV             w4, v12.s[1]
    MOV             v12.s[1], v14.s[1]
    MOV             v14.s[1], w4

    UZP1            v6.2s, v10.2s, v11.2s //x2_3

    SQSUB           v1.2s, v10.2s, v11.2s //tempr = vqsub_s32(x2_10,x3_11)
    SQADD           v5.2s, v10.2s, v11.2s //tempi = vqadd_s32(x2_10,x3_11)

    SMULL           v7.2d, v1.2s, v0.2s
    SMULL           v10.2d, v5.2s, v0.2s

    SSHR            v7.2d, v7.2d, #32   //tempr_q
    SSHR            v10.2d, v10.2d, #32 //tempi_q

    SHL             v7.4s, v7.4s, #1
    SHL             v10.4s, v10.4s, #1



    MOV             v1.s[0], v7.s[2]
    MOV             v1.s[1], v10.s[2]   //vr_i

    SQSUB           v7.2s, v6.2s, v1.2s //a2_3_v = vqsub_s32(x2_3,vr_i);
    SQADD           v4.2s, v6.2s, v1.2s //real_imag1 = vqadd_s32(x2_3,vr_i);
    SQADD           v5.2s, v14.2s, v7.2s //real_imag2 = vqadd_s32(a0_1_v,a2_3_v);

    UZP1            v1.2s, v9.2s, v13.2s //x6_7
    SQADD           v6.2s, v9.2s, v13.2s //tempr = vqadd_s32(x6_14,x7_15);
    SQSUB           v14.2s, v9.2s, v13.2s //tempi = vqsub_s32(x6_14,x7_15);

    SMULL           v9.2d, v6.2s, v0.2s
    SMULL           v13.2d, v14.2s, v0.2s

    SSHR            v9.2d, v9.2d, #32
    SSHR            v13.2d, v13.2d, #32

    SHL             v9.4s, v9.4s, #1
    SHL             v13.4s, v13.4s, #1



    MOV             v0.s[0], v9.s[2]
    MOV             v0.s[1], v13.s[2]

    SQSUB           v9.2s, v1.2s, v0.2s // a20_30_v
    SQADD           v13.2s, v1.2s, v0.2s //real_imag5


    MOV             w4, v9.s[1]
    MOV             v9.s[1], v13.s[1]
    MOV             v13.s[1], w4

    SQADD           v6.2s, v9.2s, v8.2s //real_imag3

    ST1             {v3.s}[0], [x1], #4
    ST1             {v4.s}[0], [x1], #4
    ST1             {v5.s}[0], [x1], #4
    ST1             {v6.s}[0], [x1], #4
    ST1             {v12.s}[0], [x1], #4
    ST1             {v13.s}[0], [x1], #4

    ST1             {v3.s}[1], [x2], #4
    ST1             {v4.s}[1], [x2], #4
    ST1             {v5.s}[1], [x2], #4
    ST1             {v6.s}[1], [x2], #4
    ST1             {v12.s}[1], [x2], #4
    ST1             {v13.s}[1], [x2], #4
    //ST4 {v3.s,v4.s,v5.s,v6.s}[0],[x1],x5
    //ST4 {v3.s,v4.s,v5.s,v6.s}[1],[x2],x5
    //ST2 {v12.s,v13.s}[0],[x1]
    //ST2 {v12.s,v13.s}[1],[x2]
    pop_v_regs
    ret







