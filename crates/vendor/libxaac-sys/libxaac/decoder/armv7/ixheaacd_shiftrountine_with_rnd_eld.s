.text
.p2align 2
.global ixheaacd_shiftrountine_with_rnd_eld
.type ixheaacd_shiftrountine_with_rnd_eld, %function

ixheaacd_shiftrountine_with_rnd_eld:
    STMFD           sp!, {r4-r12, r14}
    MOV             r4, #0x1f
    ADD             r12, r2, r3, LSL #1
    MOV             r9, #0x8000
    SUBS            r3, r3, #1
    BMI             S_WITH_R_L6

S_WITH_R_L5:
    LDR             r5, [r1, r3, LSL #2] @i2 = qmfImag[j]
    LDR             r7, [r0, r3, LSL #2] @r2 = qmfReal[j]
    LDR             r14, [r0], #4       @r1 = *qmfReal
    LDR             r10, [r1], #4       @i1 = *qmfImag

    ADD             r6, r5, r7          @*qmfImag++ = add32(i2, r2)
    MVN             r6, r6              @negate32(add32(i2, r2))
    ADD             r6, r6 , #1

    @SUB      r5,r5,r7          @qmfReal[j] = sub32(i2, r2)
    SUB             r5, r7, r5          @qmfReal[j] = sub32(r2, i2)

    ADD             r7, r10, r14        @qmfImag[j] = add32(i1, r1)
    MVN             r7, r7              @negate32(add32(i1, r1))
    ADD             r7, r7 , #1

    @SUB      r4,r10,r14            @*qmfReal++ = sub32(i1, r1)
    SUB             r4, r14, r10        @*qmfReal++ = sub32(r1, i1)

    @STR      r7,[r1,r3,LSL #2]
    @STR      r5,[r0,r3,LSL #2]
    @STR      r6,[r1],#4
    @STR      r4,[r0],#4



    @LDRD     r4,[r0],#8            @DEBUG

    @LDRD     r6,[r1],#8
    MOVS            r10, r4, ASR #0x16  @Right shift by 22 to check the overflow ( is not AAC_ELD right shifted by 21)
    CMNLT           r10, #1             @Check r4 is overflow or not

    MOVLT           r4, #0x80000000     @saturate value if r4 is overflowed
    MVNGT           r4, #0x80000000
    MOVEQ           r4, r4, LSL #9      @shift by 9(hardcoded value) if not AAC_ELD left shifted by 10

    MOVS            r10, r5, ASR #0x16
    QADD            r4, r4, r9
    CMNLT           r10, #1
    MOV             r4, r4, ASR #16
    MOVLT           r5, #0x80000000
    MVNGT           r5, #0x80000000
    MOVEQ           r5, r5, LSL #9
    MOV             r14, r3, lsl #1


    MOVS            r10, r6, ASR #0x16
    QADD            r5, r5, r9
    CMNLT           r10, #1
    MOV             r5, r5, ASR #16
    MOVLT           r6, #0x80000000
    @STRH     r5,[r2],#2
    STRH            r5, [r2, r14]
    MVNGT           r6, #0x80000000
    MOVEQ           r6, r6, LSL #9

    MOVS            r10, r7, ASR #0x16
    QADD            r6, r6, r9
    CMNLT           r10, #1
    MOV             r6, r6, ASR #16
    MOVLT           r7, #0x80000000
    MVNGT           r7, #0x80000000
    MOVEQ           r7, r7, LSL #9

    QADD            r7, r7, r9
    STRH            r4, [r2], #2

    MOV             r7, r7, ASR #16

    @STRH     r7,[r12],#2
    STRH            r7, [r12, r14]
    SUBS            r3, r3, #2
    STRH            r6, [r12], #2
    BGE             S_WITH_R_L5
S_WITH_R_L6:
    LDMFD           sp!, {r4-r12, r15}



