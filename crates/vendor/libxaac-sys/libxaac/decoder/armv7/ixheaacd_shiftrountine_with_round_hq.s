        .code 32
       .eabi_attribute 24, 1            @Tag_ABI_align_needed
       .eabi_attribute 25, 1            @Tag_ABI_align_preserved
.text
.p2align 2
    .global ixheaacd_shiftrountine_with_rnd_hq
    .type ixheaacd_shiftrountine_with_rnd_hq, %function
ixheaacd_shiftrountine_with_rnd_hq:

    STMFD           sp!, {r4-r12, r14}
    ADD             r12, r2, r3, LSL #2
    MOV             r9, #0x8000
    SUBS            r3, r3, #1
    BMI             S_WITH_R_L6

S_WITH_R_L5:
    LDR             r5, [r1, r3, LSL #2]
    LDR             r7, [r0, r3, LSL #2]
    LDR             r14, [r0], #4
    LDR             r10, [r1], #4

    ADD             r6, r5, r7
    SUB             r5, r5, r7
    ADD             r7, r10, r14
    SUB             r4, r10, r14











    MOVS            r10, r4, ASR #0x19
    CMNLT           r10, #1

    MOVLT           r4, #0x80000000
    MVNGT           r4, #0x80000000
    MOVEQ           r4, r4, LSL #6

    MOVS            r10, r5, ASR #0x19
    CMNLT           r10, #1
    MOVLT           r5, #0x80000000
    MVNGT           r5, #0x80000000
    MOVEQ           r5, r5, LSL #6
    MOV             r14, r3, lsl #2


    MOVS            r10, r6, ASR #0x19
    CMNLT           r10, #1
    MOVLT           r6, #0x80000000

    STR             r5, [r2, r14]
    MVNGT           r6, #0x80000000
    MOVEQ           r6, r6, LSL #6

    MOVS            r10, r7, ASR #0x19
    CMNLT           r10, #1

    MOVLT           r7, #0x80000000
    MVNGT           r7, #0x80000000
    MOVEQ           r7, r7, LSL #6

    STR             r4, [r2], #4



    STR             r7, [r12, r14]
    SUBS            r3, r3, #2
    STR             r6, [r12], #4
    BGE             S_WITH_R_L5
S_WITH_R_L6:
    LDMFD           sp!, {r4-r12, r15}
