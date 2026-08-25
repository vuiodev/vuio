//.include "ihevc_neon_macros.s"
.macro push_v_regs
    stp             X8, X9, [sp, #-16]!
    stp             X10, X11, [sp, #-16]!
    stp             X12, X13, [sp, #-16]!
    stp             X14, X15, [sp, #-16]!
    stp             X16, X17, [sp, #-16]!
    stp             x19, x20, [sp, #-16]!
    stp             x21, x22, [sp, #-16]!
    stp             X29, X30, [sp, #-16]!
.endm
.macro pop_v_regs
    ldp             X29, X30, [sp], #16
    ldp             x21, x22, [sp], #16
    ldp             x19, x20, [sp], #16
    ldp             X16, X17, [sp], #16
    ldp             X14, X15, [sp], #16
    ldp             X12, X13, [sp], #16
    ldp             X10, X11, [sp], #16
    ldp             X8, X9, [sp], #16
.endm

.text
.p2align 2
.global ixheaacd_scale_factor_process_armv8

ixheaacd_scale_factor_process_armv8:

    push_v_regs

    MOV             x9, x4

    MOV             x21, x6
    MOV             x22, x7
    CMP             x2, #0              // Tbands

    BGT             lbl17

    pop_v_regs
    ret
lbl17:
    MOV             x10, #0
    CMP             x5, #2
    BGT             ADD_34
    MOV             x11, #0x25
    B               TBANDS_LOOP
ADD_34:
    MOV             x11, #0x22
    // MOV         x11, #0x25 // temp=37

TBANDS_LOOP:
    LDRSH           x5, [x1], #2        // scale_factor = *Scfactor++;
    LDRB            w4, [x3], #1        //Offset [1]
    sxtw            x4, w4


    CMP             x5, #0x18           //if(scale_factor < 24)
    BGE             SCALE_FACTOR_GE_12  //

    CMP             x4, #0
    BLE             OFFSET_ZERO

SCALE_FACTOR_LT_12:

    STR             x10, [x0], #8
    STR             x10, [x0], #8
    SUBS            x4, x4, #4
    BGT             SCALE_FACTOR_LT_12
    B               OFFSET_ZERO

SCALE_FACTOR_GE_12:

    SUBS            x6, x11, x5, ASR #2 // 37-(scale_factor >> 2)
    AND             x5, x5, #3          // scale_factor & 0x0003

    //ADD x5,x9,x5,LSL #1 ; scale_table_ptr[(scale_factor & 0x0003)];
    LDR             w5, [x9, x5, LSL #2] // scale_short = scale_table_ptr[(scale_factor & 0x0003)];
    sxtw            x5, w5
    AND             w17, w5, #0x0000FFFF
    sxth            w17, w17            //16-bit value stored as 32-bit,so SMULWB can still be used
    BLE             SHIFT_LE_ZERO       // if shift less than or equal to zero

    SUB             x14, x6, #1         //dont do that extra LSL #1 in SMULWB

SHIFT_POSITIVE: //loop over sfbWidth a multiple of 4
    LDP             w6, w7 , [x0, #0]   // temp1 = *x_invquant
    LDP             w19, w20, [x0, #8]

    //SMULWB      x6, x6, x5 // buffex1 = mult32x16in32(temp1, scale_short);
    SMULL           x6, w6, w17
    SMULL           x7, w7, w17
    SMULL           x19, w19, w17
    SMULL           x20, w20, w17

    ASR             x6, x6, #16
    ASR             x7, x7 , #16
    ASR             x19, x19 , #16
    ASR             x20, x20 , #16

    ASR             x6, x6, x14         // buffex1 = shx32(buffex1, shift);
    ASR             x7, x7, x14
    ASR             x19, x19, x14
    ASR             x20, x20, x14

    stp             w6, w7, [x0], #8
    stp             w19, w20, [x0], #8

    SUBS            x4, x4, #4

    BGT             SHIFT_POSITIVE
    B               OFFSET_ZERO
SHIFT_LE_ZERO:

    //RSBS        x14, x6, #0 //-shift
    NEGS            x14, x6
    BGT             SHIFT_NEGTIVE1

SHIFT_ZERO: //loop over sfbWidth a multiple of 4
    LDP             w6, w7, [x0, #0]    // temp1 = *x_invquant;

    //SMULWB      x6, x6, x5 // buffex1 = mult32x16in32(temp1, scale_short);
    SMULL           x6, w6, w17
    SMULL           x7, w7, w17

    ASR             x6, x6, #16
    ASR             x7, x7, #16

    LSL             x6, x6, #1
    LSL             x7, x7, #1

    STP             w6, w7, [x0], #8    // *x_invquant++ = buffex1;

    SUBS            x4, x4, #2

    BGT             SHIFT_ZERO
    B               OFFSET_ZERO

SHIFT_NEGTIVE1:
    SUB             x14, x14, #1
SHIFT_NEGTIVE: //;loop over sfbWidth a multiple of 4

    LDP             w6, w7, [x0, #0]
    LSL             w6, w6, w14         // buffex1 = shl32(buffex1, shift-1);
    LSL             w7, w7, w14         // buffex1 = shl32(buffex1, shift-1);

    //SMULWB      x6, x6, x5 // buffex1 = mult32x16in32(temp1, scale_short);
    SMULL           x6, w6, w17
    SMULL           x7, w7, w17
    ASR             x6, x6, #16
    ASR             x7, x7, #16

    LSL             x6, x6, #2          // shl for fixmul_32x16b and shl32(buffer,1)
    LSL             x7, x7, #2          // shl for fixmul_32x16b and shl32(buffer,1)

    STP             w6, w7, [x0], #8    // *x_invquant++ = buffex1;

    SUBS            x4, x4, #2

    BGT             SHIFT_NEGTIVE

OFFSET_ZERO:
    SUBS            x2, x2, #1
    BGT             TBANDS_LOOP

    pop_v_regs
    ret
