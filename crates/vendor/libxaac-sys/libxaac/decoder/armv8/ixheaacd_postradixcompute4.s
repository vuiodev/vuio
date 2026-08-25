//.include "ihevc_neon_macros.s"
.macro push_v_regs
    stp             x8, x9, [sp, #-16]!
    stp             x10, X11, [sp, #-16]!
    stp             X12, X13, [sp, #-16]!
    stp             X14, X15, [sp, #-16]!
    stp             X29, X30, [sp, #-16]!
.endm
.macro pop_v_regs
    ldp             X29, X30, [sp], #16
    ldp             X14, X15, [sp], #16
    ldp             X12, X13, [sp], #16
    ldp             X10, X11, [sp], #16
    ldp             X8, X9, [sp], #16
.endm

.text
.p2align 2
        .global ixheaacd_postradixcompute4


ixheaacd_postradixcompute4:

    // STMFD sp!, {x4-x12, x14}
    push_v_regs
    //SUB         sp, sp, #16

    //HARD CODED for FFT Length of 16
    // x3 is always 16


    //SUB         x4, x3, #2              ; y to y offset calculated
    //MOV         x4, #14
    //STR         x4, [sp, #8]            ; (npoints / 2)*4bytes - 4bytes

    //STR         x0, [sp, #12]           ; (3*(npoints/2))*4bytes - 4bytes
                                        // x0 to x2 offset (npoints / 2)*4bytes
    ADD             x4, x1, x3, lsl #1  // x1 -> x0, x4 -> x2
    MOV             x3, #2


POSTRADIX4_START:

//    LDMIA       x1!, {x5-x12}               // x_0 :x_7

    LDP             w5, w6, [x1], #8    // x_0 :x_1
    LDP             w7, w8, [x1], #8    // x_2 :x_3
    LDP             w9, w10, [x1], #8   // x_4 :x_5
    LDP             w11, w12, [x1], #8  // x_6 :x_7

    ADD             w14, w5, w9         // xh0_0 = x_0 + x_4
    SUB             w5, w5, w9          // xl0_0 = x_0 - x_4

    ADD             w9, w6, w10         // xh1_0 = x_1 + x_5
    SUB             w6, w6, w10         // xl1_0 = x_1 - x_5

    ADD             w10, w7, w11        // xh0_1 = x_2 + x_6
    SUB             w7, w7, w11         // xl0_1 = x_2 - x_6

    ADD             w11, w8, w12        // xh1_1 = x_3 + x_7
    SUB             w8, w8, w12         // xl1_1 = x_3 - x_7

    ADD             w12, w14, w10       // n00 = xh0_0 + xh0_1
    SUB             w14, w14, w10       // n20 = xh0_0 - xh0_1

    ADD             w10, w9, w11        // n01 = xh1_0 + xh1_1
    SUB             w9, w9, w11         // n21 = xh1_0 - xh1_1

    ADD             w11, w5, w8         // n10 = xl0_0 + xl1_1
    SUB             w5, w5, w8          // n30 = xl0_0 - xl1_1

    ADD             w8, w6, w7          // n31 = xl1_0 + xl0_1
    SUB             w6, w6, w7          // n11 = xl1_0 - xl0_1


    STR             w12, [x0], #4       // y0[h2] = n00, x7 -> y0[h2 + 1]

    STR             w10, [x0], #14<<1   // y0[h2 + 1] = n01, x7 -> y1[h2]

    STR             w11, [x0], #4       // y1[h2] = n10, x7 -> y1[h2 + 1]
    STR             w6 , [x0], #14<<1   // y1[h2 + 1] = n11, x7 -> y2[h2]

    STR             w14, [x0], #4       // y2[h2] = n20, x7 -> y2[h2 + 1]
    STR             w9 , [x0], #14<<1   // y2[h2 + 1] = n21, x7 -> y3[h2]

    STR             w5, [x0], #4        // y3[h2] = n30, x7 -> y3[h2 + 1]
    STR             w8, [x0], #0        // y3[h2 + 1] = n31, x7 -> y0[h2+2]

//    LDMIA       x4!, {x5-x12}               // x_0 :x_7

    LDP             w5, w6, [x4], #8    // x_8 :x_8
    LDP             w7, w8, [x4], #8    // x_a :x_b
    LDP             w9, w10, [x4], #8   // x_c :x_d
    LDP             w11, w12, [x4], #8  // x_e :x_f

    SUB             x0, x0, #92         // #4*3 + #14<<1 * 3 - 8


    ADD             w14, w5, w9
    SUB             w5, w5, w9

    ADD             w9, w6, w10
    SUB             w6, w6, w10

    ADD             w10, w7, w11
    SUB             w7, w7, w11

    ADD             w11, w8, w12
    SUB             w8, w8, w12

    ADD             w12, w14, w10
    SUB             w14, w14, w10

    ADD             w10, w9, w11
    SUB             w9, w9, w11

    ADD             w11, w5, w8
    SUB             w5, w5, w8

    ADD             w8, w6, w7
    SUB             w6, w6, w7

    STR             w12, [x0], #4
    STR             w10, [x0], #14<<1

    STR             w11, [x0], #4
    STR             w6, [x0], #14<<1

    STR             w14, [x0], #4
    STR             w9, [x0], #14<<1


    STR             w5, [x0], #4
    STR             w8, [x0], #0

    ADD             x1, x1, #1 << 5     // x0 += (Word32) npoints >> 1
    ADD             x4, x4, #1 << 5     // x2 += (Word32) npoints >> 1
    SUB             x0, x0, #100-8

    SUBS            w3, w3, #1

    BGT             POSTRADIX4_START

    // LDMFD sp!, {x4-x12, x15}
    pop_v_regs
    ret


