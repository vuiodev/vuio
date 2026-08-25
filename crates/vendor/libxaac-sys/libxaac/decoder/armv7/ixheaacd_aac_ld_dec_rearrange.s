.text
.p2align 2
.global ia_aac_ld_dec_rearrange_armv7

ia_aac_ld_dec_rearrange_armv7:
    STMFD           r13!, {r4 - r12, r14}
    @ASR    r2,r2,#3            @
    MOV             R2, R2, ASR #3


LOOP_REARRANGE:
    LDRB            r4, [r3], #1        @ idx = mdct_tables_ptr->re_arr_tab[n]
    LDRB            r5, [r3], #1        @ idx = mdct_tables_ptr->re_arr_tab[n]
    LDRB            r6, [r3], #1        @ idx = mdct_tables_ptr->re_arr_tab[n]
    LDRB            r7, [r3], #1        @ idx = mdct_tables_ptr->re_arr_tab[n]
    LDRB            r8, [r3], #1        @ idx = mdct_tables_ptr->re_arr_tab[n]
    LDRB            r9, [r3], #1        @ idx = mdct_tables_ptr->re_arr_tab[n]
    LDRB            r10, [r3], #1       @ idx = mdct_tables_ptr->re_arr_tab[n]
    LDRB            r11, [r3], #1       @ idx = mdct_tables_ptr->re_arr_tab[n]
    ADD             r4, r0, r4, lsl #3
    ADD             r5, r0, r5, lsl #3
    ADD             r6, r0, r6, lsl #3
    ADD             r7, r0, r7, lsl #3
    ADD             r8, r0, r8, lsl #3
    ADD             r9, r0, r9, lsl #3
    ADD             r10, r0, r10, lsl #3
    ADD             r11, r0, r11, lsl #3
    LDMIA           r4, {r12, r14}      @ r12 = inp[idx] and r14 = inp[idx+1]
    STMIA           r1!, {r12, r14}     @ *buf1++ = inp[idx] and *buf1++ = inp[idx+1]
    LDMIA           r5, {r12, r14}      @ r12 = inp[idx] and r14 = inp[idx+1]
    STMIA           r1!, {r12, r14}     @ *buf1++ = inp[idx] and *buf1++ = inp[idx+1]
    LDMIA           r6, {r12, r14}      @ r12 = inp[idx] and r14 = inp[idx+1]
    STMIA           r1!, {r12, r14}     @ *buf1++ = inp[idx] and *buf1++ = inp[idx+1]
    LDMIA           r7, {r12, r14}      @ r12 = inp[idx] and r14 = inp[idx+1]
    STMIA           r1!, {r12, r14}     @ *buf1++ = inp[idx] and *buf1++ = inp[idx+1]
    LDMIA           r8, {r12, r14}      @ r12 = inp[idx] and r14 = inp[idx+1]
    STMIA           r1!, {r12, r14}     @ *buf1++ = inp[idx] and *buf1++ = inp[idx+1]
    LDMIA           r9, {r12, r14}      @ r12 = inp[idx] and r14 = inp[idx+1]
    STMIA           r1!, {r12, r14}     @ *buf1++ = inp[idx] and *buf1++ = inp[idx+1]
    LDMIA           r10, {r12, r14}     @ r12 = inp[idx] and r14 = inp[idx+1]
    STMIA           r1!, {r12, r14}     @ *buf1++ = inp[idx] and *buf1++ = inp[idx+1]
    LDMIA           r11, {r12, r14}     @ r12 = inp[idx] and r14 = inp[idx+1]
    STMIA           r1!, {r12, r14}     @ *buf1++ = inp[idx] and *buf1++ = inp[idx+1]

    SUBS            r2, r2, #1
    BGT             LOOP_REARRANGE

    LDMFD           r13!, {r4 - r12, r15}


