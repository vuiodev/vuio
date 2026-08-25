.text
.p2align 2
.global ia_eld_decoder_sbr_pre_twiddle


ia_eld_decoder_sbr_pre_twiddle:



    STMFD           sp!, {r4-r12, r14}
    LDR             r4, [r0, #0]        @Xre  = *pXre
    MOV             r3, #62             @Loop count
    LDR             r5, [r1, #0]        @Xim  = *pXim

LOOP:
    LDR             r6, [r2], #4        @Load and increment pointer *pTwiddles++ Lower - cosine , higher - sine
    SUBS            r3, r3, #1          @Decrement loop count by 1

    SMULWB          r8, r4, r6          @mult32x16in32(Xre, cosine)
    LSL             r8, r8, #1          @Left shift the multiplied value by 1

    SMULWT          r10, r5, r6         @mult32x16in32( Xim , sine)

    ADD             r12, r8, r10, LSL #1 @mac32x16in32_shl( mult32x16in32_shl(Xre, cosine) , mult32x16in32_shl( Xim , sine))@


    SMULWT          r7, r4, r6          @mult32x16in32(Xre, sine)
    LDR             r4, [r0, #4]        @Load next iteration value Xre  = *pXre

    SMULWB          r9, r5, r6          @mult32x16in32(Xim, cosine)
    STR             r12, [r0], #4       @Store and increment pointer *pXre++ = re

    LSL             r9, r9, #1          @Left shift the multiplied value by 1
    LDR             r5, [r1, #4]        @Load next iteration value Xim  = *pXim


    SUB             r14, r9, r7, LSL #1 @sub32(mult32x16in32_shl(Xim, cosine) , mult32x16in32_shl(Xre, sine))

    STR             r14, [r1], #4       @Store and increment pointer *pXim++ = im

    BNE             LOOP                @Check r3 equals 0 and continue

EPILOUGE:

    LDR             r6, [r2], #4

    SMULWB          r8, r4, r6
    LSL             r8, r8, #1

    SMULWT          r10, r5, r6

    ADD             r12, r8, r10, LSL #1


    SMULWB          r9, r5, r6
    LSL             r9, r9, #1

    SMULWT          r7, r4, r6

    SUB             r14, r9, r7, LSL #1

    STR             r12, [r0], #4
    STR             r14, [r1], #4

END_LOOP:

    LDMFD           sp!, {r4-r12, pc}
