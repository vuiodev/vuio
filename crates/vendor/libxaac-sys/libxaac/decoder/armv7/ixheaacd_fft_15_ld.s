

.text
.p2align 2
.global ixheaacd_fft_15_ld_armv7

ixheaacd_fft_15_ld_armv7:

    STMFD           r13!, {r4 - r12, r14} @
    STR             r1  , [r13, #-4]!   @
    STR             r3  , [r13, #-4]!   @
    MOV             lr, r2              @ lr - fft3out
    MOV             r12, #384           @


LOOP_FFT5:
    LDRD            r2, [r0]            @ r2 = buf1a[0] and r3 = buf1a[1]
    ADD             r0, r0, r12
    LDRD            r4, [r0]            @ r4 = buf1a[2] and r5 = buf1a[3]
    ADD             r0, r0, r12
    LDRD            r6, [r0]            @ r6 = buf1a[4] and r7 = buf1a[5]
    ADD             r0, r0, r12
    LDRD            r8, [r0]            @ r8 = buf1a[6] and r9 = buf1a[7]
    ADD             r0, r0, r12
    LDRD            r10, [r0]           @ r10 = buf1a[8] and r11 = buf1a[9]


    ADD             r1, r4, r10         @ r1 = buf1a[2] + buf1a[8]
    SUB             r4, r4, r10         @ r4 = buf1a[2] - buf1a[8]@
    MOVW            r10, #0xB000
    MOVT            r10, #0x478E
    ADD             r12, r6, r8         @ r3 = buf1a[4] + buf1a[6]
    SUB             r8, r6, r8          @ r2 = buf1a[4] - buf1a[6]

    SUB             r6, r1, r12         @ (r1 - r3)
    SMULWT          r6, r6, r10         @ t = mult32x16in32_shl((r1 - r3), C54)
    ADD             r1, r1, r12         @ r1 = r1 + r3@
    ADD             r2, r2, r1          @ temp1 = inp[0] + r1@
    SMULWB          r1, r1, r10         @ mult32_shl(r1, C55)
    ADD             r1, r2, r1, lsl #2  @ r1 = temp1 + ((mult32_shl(r1, C55)) << 1)@
    MOVW            r10, #0x9D84
    MOVT            r10, #0x79BC
    STR             r2, [lr], #4        @ *buf2++ = temp1@

    SUB             r12, r1, r6, LSL #1 @ r3 = r1 - t@
    ADD             r1, r1, r6, LSL #1  @ r1 = r1 + t@

    ADD             r2, r4, r8          @ (r4 + r2)
    SMULWT          r2, r2, r10         @ t = mult32_shl((r4 + r2), C51)@

    @LSL     r2, r2, #1
    MOV             r2, r2, LSL #1

    SMULWB          r4, r4, r10         @ mult32_shl(r4, C52)
    MOVW            r10, #0xD180
    MOVT            r10, #0xFFFF
    ADD             r4, r2, r4, LSL #2  @ r4 = t + (mult32_shl(r4, C52) << 1)@

    SMULWB          r8, r8, r10         @ mult32_shl(r2, C53)
    ADD             r2, r2, r8, LSL #1  @ r2 = t + mult32_shl(r2, C53)@

    ADD             r6, r5, r11         @ s1 = buf1a[3] + buf1a[9]
    SUB             r8, r5, r11         @ s4 = buf1a[3] - buf1a[9]
    MOVW            r10, #0xB000
    MOVT            r10, #0x478E
    ADD             r5, r7, r9          @ s3 = buf1a[5] + buf1a[7]@
    SUB             r7, r7, r9          @ s2 = buf1a[5] + buf1a[7]@


    SUB             r9, r6, r5          @ (s1 - s3)
    SMULWT          r9, r9, r10         @ t = mult32x16in32_shl((s1 - s3), C54)
    ADD             r6, r6, r5          @ s1 = s1 + s3@
    ADD             r3, r3, r6          @ temp2 = buf1a[1] + s1
    SMULWB          r6, r6, r10         @ mult32_shl(s1, C55)
    ADD             r6, r3, r6, lsl #2  @ s1 = temp1 + ((mult32_shl(s1, C55)) << 1)@
    MOVW            r10, #0x9D84
    MOVT            r10, #0x79BC
    STR             r3, [lr], #4        @ *buf2++ = temp2@

    SUB             r5, r6, r9, LSL #1  @ s3 = s1 - t@
    ADD             r6, r6, r9, LSL #1  @ s1 = s1 + t@
    SUB             r0, r0, #896        @ r0 -inp[160]

    ADD             r11, r7, r8         @ (s4 + s2)
    SMULWT          r11, r11, r10       @ t = mult32_shl((s4 + s2), C51)@
    @LSL     r11, r11, #1           @
    MOV             r11, r11, LSL #1


    SMULWB          r8, r8, r10         @ mult32_shl(s4, C52)
    MOVW            r10, #0xD180
    MOVT            r10, #0xFFFF
    ADD             r8, r11, r8, LSL #2 @ s4 = t + (mult32_shl(s4, C52) << 1)@

    SMULWB          r7, r7, r10         @ mult32_shl(s2, C53)
    ADD             r7, r11, r7, LSL #1 @ s2 = t + mult32_shl(s2, C53)@


    ADD             r3, r1, r7          @ buf2[2] = r1 + s2
    SUB             r9, r6, r2          @ buf2[3] = s1 - r2
    SUB             r10, r12, r8        @ buf2[4] = r3 - s4
    ADD             r11, r5, r4         @ buf2[5] = s3 + r4
    ADD             r12, r12, r8        @ buf2[6] = r3 + s4
    SUB             r4, r5, r4          @ buf2[7] = s3 - r4
    SUB             r5, r1, r7          @ buf2[8] = r1 - s2
    ADD             r6, r6, r2          @ buf2[9] = s1 + r2
    STMIA           lr!, {r3, r9-r12}   @

    MOV             r12, #384           @
    MOVW            r1, #0xFA00
    MOVT            r1, #0xFFFF

    STMIA           lr!, {r4-r6}        @


    LDRD            r2, [r0]            @ r2 = buf1a[0] and r3 = buf1a[1]
    ADD             r0, r0, r12
    LDRD            r4, [r0]            @ r4 = buf1a[2] and r5 = buf1a[3]
    ADD             r0, r0, r12
    LDRD            r6, [r0]            @ r6 = buf1a[4] and r7 = buf1a[5]
    ADD             r0, r0, r12
    LDRD            r8, [r0]            @ r8 = buf1a[6] and r9 = buf1a[7]
    ADD             r0, r0, r1
    LDRD            r10, [r0]           @ r10 = buf1a[8] and r11 = buf1a[9]
    ADD             r0, r0, #1024       @ r0 -inp[320]

    ADD             r1, r4, r10         @ r1 = buf1a[2] + buf1a[8]
    SUB             r4, r4, r10         @ r4 = buf1a[2] - buf1a[8]@
    MOVW            r10, #0xB000
    MOVT            r10, #0x478E
    ADD             r12, r6, r8         @ r3 = buf1a[4] + buf1a[6]
    SUB             r8, r6, r8          @ r2 = buf1a[4] - buf1a[6]

    SUB             r6, r1, r12         @ (r1 - r3)
    SMULWT          r6, r6, r10         @ t = mult32x16in32_shl((r1 - r3), C54)
    ADD             r1, r1, r12         @ r1 = r1 + r3@
    ADD             r2, r2, r1          @ temp1 = inp[0] + r1@
    SMULWB          r1, r1, r10         @ mult32_shl(r1, C55)
    ADD             r1, r2, r1, lsl #2  @ r1 = temp1 + ((mult32_shl(r1, C55)) << 1)@
    MOVW            r10, #0x9D84
    MOVT            r10, #0x79BC
    STR             r2, [lr], #4        @ *buf2++ = temp1@

    SUB             r12, r1, r6, LSL #1 @ r3 = r1 - t@
    ADD             r1, r1, r6, LSL #1  @ r1 = r1 + t@

    ADD             r2, r4, r8          @ (r4 + r2)
    SMULWT          r2, r2, r10         @ t = mult32_shl((r4 + r2), C51)@
    @LSL     r2, r2, #1
    MOV             r2, r2, LSL #1


    SMULWB          r4, r4, r10         @ mult32_shl(r4, C52)
    MOVW            r10, #0xD180
    MOVT            r10, #0xFFFF
    ADD             r4, r2, r4, LSL #2  @ r4 = t + (mult32_shl(r4, C52) << 1)@

    SMULWB          r8, r8, r10         @ mult32_shl(r2, C53)
    ADD             r2, r2, r8, LSL #1  @ r2 = t + mult32_shl(r2, C53)@

    ADD             r6, r5, r11         @ s1 = buf1a[3] + buf1a[9]
    SUB             r8, r5, r11         @ s4 = buf1a[3] - buf1a[9]
    MOVW            r10, #0xB000
    MOVT            r10, #0x478E
    ADD             r5, r7, r9          @ s3 = buf1a[5] + buf1a[7]@
    SUB             r7, r7, r9          @ s2 = buf1a[5] + buf1a[7]@


    SUB             r9, r6, r5          @ (s1 - s3)
    SMULWT          r9, r9, r10         @ t = mult32x16in32_shl((s1 - s3), C54)
    ADD             r6, r6, r5          @ s1 = s1 + s3@
    ADD             r3, r3, r6          @ temp2 = buf1a[1] + s1
    SMULWB          r6, r6, r10         @ mult32_shl(s1, C55)
    ADD             r6, r3, r6, lsl #2  @ s1 = temp1 + ((mult32_shl(s1, C55)) << 1)@
    MOVW            r10, #0x9D84
    MOVT            r10, #0x79BC
    STR             r3, [lr], #4        @ *buf2++ = temp2@


    SUB             r5, r6, r9, LSL #1  @ s3 = s1 - t@
    ADD             r6, r6, r9, LSL #1  @ s1 = s1 + t@

    ADD             r11, r7, r8         @ (s4 + s2)
    SMULWT          r11, r11, r10       @ t = mult32_shl((s4 + s2), C51)@
    @LSL     r11, r11, #1
    MOV             r11, r11, LSL #1

    SMULWB          r8, r8, r10         @mult32_shl(s4, C52)
    MOVW            r10, #0xD180
    MOVT            r10, #0xFFFF
    ADD             r8, r11, r8, LSL #2 @s4 = t + (mult32_shl(s4, C52) << 1)@

    SMULWB          r7, r7, r10         @mult32_shl(s2, C53)
    ADD             r7, r11, r7, LSL #1 @s2 = t + mult32_shl(s2, C53)@

    ADD             r3, r1, r7          @buf2[2] = r1 + s2
    SUB             r9, r6, r2          @buf2[3] = s1 - r2
    SUB             r10, r12, r8        @buf2[4] = r3 - s4
    ADD             r11, r5, r4         @buf2[5] = s3 + r4
    ADD             r12, r12, r8        @buf2[6] = r3 + s4
    SUB             r4, r5, r4          @buf2[7] = s3 - r4
    SUB             r5, r1, r7          @buf2[8] = r1 - s2
    ADD             r6, r6, r2          @buf2[9] = s1 + r2
    MOVW            r1, #0xFA00
    MOVT            r1, #0xFFFF

    STMIA           lr!, {r3, r9-r12}
    MOV             r12, #384           @
    STMIA           lr!, {r4-r6}        @

    LDRD            r2, [r0]            @ r2 = buf1a[0] and r3 = buf1a[1]
    ADD             r0, r0, r12
    LDRD            r4, [r0]            @ r4 = buf1a[2] and r5 = buf1a[3]
    ADD             r0, r0, r1

    LDRD            r6, [r0]            @ r6 = buf1a[4] and r7 = buf1a[5]
    ADD             r0, r0, r12
    LDRD            r8, [r0]            @ r8 = buf1a[6] and r9 = buf1a[7]
    ADD             r0, r0, r12
    LDRD            r10, [r0]           @ r10 = buf1a[8] and r11 = buf1a[9]
    ADD             r0, r0, r12

    ADD             r1, r4, r10         @ r1 = buf1a[2] + buf1a[8]
    SUB             r4, r4, r10         @ r4 = buf1a[2] - buf1a[8]@
    MOVW            r10, #0xB000
    MOVT            r10, #0x478E
    ADD             r12, r6, r8         @ r3 = buf1a[4] + buf1a[6]
    SUB             r8, r6, r8          @ r2 = buf1a[4] - buf1a[6]

    SUB             r6, r1, r12         @ (r1 - r3)
    SMULWT          r6, r6, r10         @ t = mult32x16in32_shl((r1 - r3), C54)
    ADD             r1, r1, r12         @ r1 = r1 + r3@
    ADD             r2, r2, r1          @ temp1 = inp[0] + r1@
    SMULWB          r1, r1, r10         @ mult32_shl(r1, C55)
    ADD             r1, r2, r1, lsl #2  @ r1 = temp1 + ((mult32_shl(r1, C55)) << 1)@
    MOVW            r10, #0x9D84
    MOVT            r10, #0x79BC
    STR             r2, [lr], #4        @ *buf2++ = temp1@

    SUB             r12, r1, r6, LSL #1 @ r3 = r1 - t@
    ADD             r1, r1, r6, LSL #1  @ r1 = r1 + t@

    ADD             r2, r4, r8          @ (r4 + r2)
    SMULWT          r2, r2, r10         @ t = mult32_shl((r4 + r2), C51)@
    @LSL     r2, r2, #1
    MOV             r2, r2, LSL #1

    SMULWB          r4, r4, r10         @ mult32_shl(r4, C52)
    MOVW            r10, #0xD180
    MOVT            r10, #0xFFFF
    ADD             r4, r2, r4, LSL #2  @ r4 = t + (mult32_shl(r4, C52) << 1)@

    SMULWB          r8, r8, r10         @ mult32_shl(r2, C53)
    ADD             r2, r2, r8, LSL #1  @ r2 = t + mult32_shl(r2, C53)@

    ADD             r6, r5, r11         @ s1 = buf1a[3] + buf1a[9]
    SUB             r8, r5, r11         @ s4 = buf1a[3] - buf1a[9]
    MOVW            r10, #0xB000
    MOVT            r10, #0x478E
    ADD             r5, r7, r9          @ s3 = buf1a[5] + buf1a[7]@
    SUB             r7, r7, r9          @ s2 = buf1a[5] + buf1a[7]@

    SUB             r9, r6, r5          @ (s1 - s3)
    SMULWT          r9, r9, r10         @ t = mult32x16in32_shl((s1 - s3), C54)
    ADD             r6, r6, r5          @ s1 = s1 + s3@
    ADD             r3, r3, r6          @ temp2 = buf1a[1] + s1
    SMULWB          r6, r6, r10         @ mult32_shl(s1, C55)
    ADD             r6, r3, r6, lsl #2  @ s1 = temp1 + ((mult32_shl(s1, C55)) << 1)@
    MOVW            r10, #0x9D84
    MOVT            r10, #0x79BC
    STR             r3, [lr], #4        @ *buf2++ = temp2@

    SUB             r5, r6, r9, LSL #1  @ s3 = s1 - t@
    ADD             r6, r6, r9, LSL #1  @ s1 = s1 + t@

    ADD             r11, r7, r8         @ (s4 + s2)
    SMULWT          r11, r11, r10       @ t = mult32_shl((s4 + s2), C51)@
    @LSL     r11, r11, #1           @
    MOV             r11, r11, LSL #1

    SMULWB          r8, r8, r10         @mult32_shl(s4, C52)
    MOVW            r10, #0xD180
    MOVT            r10, #0xFFFF
    ADD             r8, r11, r8, LSL #2 @s4 = t + (mult32_shl(s4, C52) << 1)@


    SMULWB          r7, r7, r10         @mult32_shl(s2, C53)
    ADD             r7, r11, r7, LSL #1 @s2 = t + mult32_shl(s2, C53)@

    ADD             r3, r1, r7          @buf2[2] = r1 + s2
    SUB             r9, r6, r2          @buf2[3] = s1 - r2
    SUB             r10, r12, r8        @buf2[4] = r3 - s4
    ADD             r11, r5, r4         @buf2[5] = s3 + r4
    ADD             r12, r12, r8        @buf2[6] = r3 + s4
    SUB             r4, r5, r4          @buf2[7] = s3 - r4
    SUB             r5, r1, r7          @buf2[8] = r1 - s2
    ADD             r6, r6, r2          @buf2[9] = s1 + r2

    STMIA           lr!, {r3, r9-r12}
    STMIA           lr!, {r4-r6}        @

    SUB             lr, lr, #120        @
    MOVW            r12, # 28378        @
    LDMFD           r13!, {r10, r11}    @


LOOP_FFT3:
    LDRD            r0, [lr]            @ r0 = fft3outptr[0] and r1 = fft3outptr[1]
    LDRD            r2, [lr, #40]       @ r2 = fft3outptr[10] and r3 = fft3outptr[11]
    LDRD            r4, [lr, #80]       @ r4 = fft3outptr[20] and r5 = fft3outptr[21]
    ADD             lr, lr, #8          @

    ADD             r6, r0, r2          @ X01r = add32(buf1[0], buf1[2])
    ADD             r7, r1, r3          @ X01i = add32(buf1[1], buf1[3])

    ADD             r8, r2, r4          @ add_r = add32(buf1[2], buf1[4])
    ADD             r9, r3, r5          @ add_i = add32(buf1[3], buf1[5])

    SUB             r2, r2, r4          @ sub_r = sub32(buf1[2], buf1[4])@
    SUB             r3, r3, r5          @ sub_i = sub32(buf1[3], buf1[5])@

    @ASR        r8, r8, #1          @ p1 = add_r >> 1@
    MOV             r8, r8, ASR #1

    @ASR        r9, r9, #1          @ p4 = add_i >> 1@
    MOV             r9, r9, ASR #1

    SMULWB          r3, r3, r12         @ p2 = mult32x16in32_shl(sub_i, sinmu)@
    SMULWB          r2, r2, r12         @ p3 = mult32x16in32_shl(sub_r, sinmu)@

    SUB             r0, r0, r8          @ temp = sub32(buf1a[0], p1)@
    ADD             r8, r1, r2, LSL #1  @ temp1 = add32(buf1a[1], p3)@
    SUB             r2, r1, r2, LSL #1  @ temp2 = sub32(buf1a[1], p3)@

    ADD             r4, r6, r4          @ add32(X01r, buf1a[4])@
    ADD             r5, r7, r5          @ add32(X01i, buf1a[5])@
    ADD             r6, r0, r3, LSL #1  @ add32(temp, p2)@
    SUB             r7, r2, r9          @ sub32(temp2, p4)@
    SUB             r9, r8, r9          @ sub32(temp1, p4)@
    SUB             r8, r0, r3, LSL #1  @ sub32(temp, p2)@

    MOV             r3, r11             @
    LDRB            r0, [r10], #1       @
    LDRB            r1, [r10], #1       @
    LDRB            r2, [r10], #1       @
    ADD             r0, r11, r0, lsl #3 @
    ADD             r1, r11, r1, lsl #3 @
    ADD             r2, r11, r2, lsl #3 @
    STRD            r4, [r0]            @
    STRD            r6, [r1]            @
    STRD            r8, [r2]            @

    LDRD            r0, [lr]            @ r0 = fft3outptr[0] and r1 = fft3outptr[1]
    LDRD            r2, [lr, #40]       @ r2 = fft3outptr[10] and r3 = fft3outptr[11]
    LDRD            r4, [lr, #80]       @ r4 = fft3outptr[20] and r5 = fft3outptr[21]
    ADD             lr, lr, #8          @


    ADD             r6, r0, r2          @ X01r = add32(buf1[0], buf1[2])
    ADD             r7, r1, r3          @ X01i = add32(buf1[1], buf1[3])

    ADD             r8, r2, r4          @ add_r = add32(buf1[2], buf1[4])
    ADD             r9, r3, r5          @ add_i = add32(buf1[3], buf1[5])

    SUB             r2, r2, r4          @ sub_r = sub32(buf1[2], buf1[4])@
    SUB             r3, r3, r5          @ sub_i = sub32(buf1[3], buf1[5])@

    @ASR        r8, r8, #1          @ p1 = add_r >> 1@
    MOV             r8, r8, ASR #1
    @ASR        r9, r9, #1          @ p4 = add_i >> 1@
    MOV             r9, r9, ASR #1

    SMULWB          r3, r3, r12         @ p2 = mult32x16in32_shl(sub_i, sinmu)@
    SMULWB          r2, r2, r12         @ p3 = mult32x16in32_shl(sub_r, sinmu)@

    SUB             r0, r0, r8          @ temp = sub32(buf1a[0], p1)@
    ADD             r8, r1, r2, LSL #1  @ temp1 = add32(buf1a[1], p3)@
    SUB             r2, r1, r2, LSL #1  @ temp2 = sub32(buf1a[1], p3)@

    ADD             r4, r6, r4          @ add32(X01r, buf1a[4])@
    ADD             r5, r7, r5          @ add32(X01i, buf1a[5])@
    ADD             r6, r0, r3, LSL #1  @ add32(temp, p2)@
    SUB             r7, r2, r9          @ sub32(temp2, p4)@
    SUB             r9, r8, r9          @ sub32(temp1, p4)@
    SUB             r8, r0, r3, LSL #1  @ sub32(temp, p2)@

    LDRB            r0, [r10], #1       @
    LDRB            r1, [r10], #1       @
    LDRB            r2, [r10], #1       @
    ADD             r0, r11, r0, lsl #3 @
    ADD             r1, r11, r1, lsl #3 @
    ADD             r2, r11, r2, lsl #3 @
    STRD            r4, [r0]            @
    STRD            r6, [r1]            @
    STRD            r8, [r2]            @

    LDRD            r0, [lr]            @ r0 = fft3outptr[0] and r1 = fft3outptr[1]
    LDRD            r2, [lr, #40]       @ r2 = fft3outptr[10] and r3 = fft3outptr[11]
    LDRD            r4, [lr, #80]       @ r4 = fft3outptr[20] and r5 = fft3outptr[21]
    ADD             lr, lr, #8          @


    ADD             r6, r0, r2          @ X01r = add32(buf1[0], buf1[2])
    ADD             r7, r1, r3          @ X01i = add32(buf1[1], buf1[3])

    ADD             r8, r2, r4          @ add_r = add32(buf1[2], buf1[4])
    ADD             r9, r3, r5          @ add_i = add32(buf1[3], buf1[5])

    SUB             r2, r2, r4          @ sub_r = sub32(buf1[2], buf1[4])@
    SUB             r3, r3, r5          @ sub_i = sub32(buf1[3], buf1[5])@


    @ASR        r8, r8, #1          @ p1 = add_r >> 1@
    MOV             r8, r8, ASR #1
    @ASR        r9, r9, #1          @ p4 = add_i >> 1@
    MOV             r9, r9, ASR #1

    SMULWB          r3, r3, r12         @ p2 = mult32x16in32_shl(sub_i, sinmu)@
    SMULWB          r2, r2, r12         @ p3 = mult32x16in32_shl(sub_r, sinmu)@

    SUB             r0, r0, r8          @ temp = sub32(buf1a[0], p1)@
    ADD             r8, r1, r2, LSL #1  @ temp1 = add32(buf1a[1], p3)@
    SUB             r2, r1, r2, LSL #1  @ temp2 = sub32(buf1a[1], p3)@

    ADD             r4, r6, r4          @ add32(X01r, buf1a[4])@
    ADD             r5, r7, r5          @ add32(X01i, buf1a[5])@
    ADD             r6, r0, r3, LSL #1  @ add32(temp, p2)@
    SUB             r7, r2, r9          @ sub32(temp2, p4)@
    SUB             r9, r8, r9          @ sub32(temp1, p4)@
    SUB             r8, r0, r3, LSL #1  @ sub32(temp, p2)@

    LDRB            r0, [r10], #1       @
    LDRB            r1, [r10], #1       @
    LDRB            r2, [r10], #1       @
    ADD             r0, r11, r0, lsl #3 @
    ADD             r1, r11, r1, lsl #3 @
    ADD             r2, r11, r2, lsl #3 @
    STRD            r4, [r0]            @
    STRD            r6, [r1]            @
    STRD            r8, [r2]            @

    LDRD            r0, [lr]            @ r0 = fft3outptr[0] and r1 = fft3outptr[1]
    LDRD            r2, [lr, #40]       @ r2 = fft3outptr[10] and r3 = fft3outptr[11]
    LDRD            r4, [lr, #80]       @ r4 = fft3outptr[20] and r5 = fft3outptr[21]
    ADD             lr, lr, #8          @

    ADD             r6, r0, r2          @ X01r = add32(buf1[0], buf1[2])
    ADD             r7, r1, r3          @ X01i = add32(buf1[1], buf1[3])

    ADD             r8, r2, r4          @ add_r = add32(buf1[2], buf1[4])
    ADD             r9, r3, r5          @ add_i = add32(buf1[3], buf1[5])

    SUB             r2, r2, r4          @ sub_r = sub32(buf1[2], buf1[4])@
    SUB             r3, r3, r5          @ sub_i = sub32(buf1[3], buf1[5])@

    @ASR        r8, r8, #1          @ p1 = add_r >> 1@
    MOV             r8, r8, ASR #1
    @ASR        r9, r9, #1          @ p4 = add_i >> 1@
    MOV             r9, r9, ASR #1

    SMULWB          r3, r3, r12         @ p2 = mult32x16in32_shl(sub_i, sinmu)@
    SMULWB          r2, r2, r12         @ p3 = mult32x16in32_shl(sub_r, sinmu)@

    SUB             r0, r0, r8          @ temp = sub32(buf1a[0], p1)@
    ADD             r8, r1, r2, LSL #1  @ temp1 = add32(buf1a[1], p3)@
    SUB             r2, r1, r2, LSL #1  @ temp2 = sub32(buf1a[1], p3)@

    ADD             r4, r6, r4          @ add32(X01r, buf1a[4])@
    ADD             r5, r7, r5          @ add32(X01i, buf1a[5])@
    ADD             r6, r0, r3, LSL #1  @ add32(temp, p2)@
    SUB             r7, r2, r9          @ sub32(temp2, p4)@
    SUB             r9, r8, r9          @ sub32(temp1, p4)@
    SUB             r8, r0, r3, LSL #1  @ sub32(temp, p2)@

    LDRB            r0, [r10], #1       @
    LDRB            r1, [r10], #1       @
    LDRB            r2, [r10], #1       @
    ADD             r0, r11, r0, lsl #3 @
    ADD             r1, r11, r1, lsl #3 @
    ADD             r2, r11, r2, lsl #3 @
    STRD            r4, [r0]            @
    STRD            r6, [r1]            @
    STRD            r8, [r2]            @

    LDRD            r0, [lr]            @ r0 = fft3outptr[0] and r1 = fft3outptr[1]
    LDRD            r2, [lr, #40]       @ r2 = fft3outptr[10] and r3 = fft3outptr[11]
    LDRD            r4, [lr, #80]       @ r4 = fft3outptr[20] and r5 = fft3outptr[21]

    ADD             r6, r0, r2          @ X01r = add32(buf1[0], buf1[2])
    ADD             r7, r1, r3          @ X01i = add32(buf1[1], buf1[3])

    ADD             r8, r2, r4          @ add_r = add32(buf1[2], buf1[4])
    ADD             r9, r3, r5          @ add_i = add32(buf1[3], buf1[5])

    SUB             r2, r2, r4          @ sub_r = sub32(buf1[2], buf1[4])@
    SUB             r3, r3, r5          @ sub_i = sub32(buf1[3], buf1[5])@

    @ASR        r8, r8, #1          @ p1 = add_r >> 1@
    MOV             r8, r8, ASR #1
    @ASR        r9, r9, #1          @ p4 = add_i >> 1@
    MOV             r9, r9, ASR #1

    SMULWB          r3, r3, r12         @ p2 = mult32x16in32_shl(sub_i, sinmu)@
    SMULWB          r2, r2, r12         @ p3 = mult32x16in32_shl(sub_r, sinmu)@

    SUB             r0, r0, r8          @ temp = sub32(buf1a[0], p1)@
    ADD             r8, r1, r2, LSL #1  @ temp1 = add32(buf1a[1], p3)@
    SUB             r2, r1, r2, LSL #1  @ temp2 = sub32(buf1a[1], p3)@

    ADD             r4, r6, r4          @ add32(X01r, buf1a[4])@
    ADD             r5, r7, r5          @ add32(X01i, buf1a[5])@
    ADD             r6, r0, r3, LSL #1  @ add32(temp, p2)@
    SUB             r7, r2, r9          @ sub32(temp2, p4)@
    SUB             r9, r8, r9          @ sub32(temp1, p4)@
    SUB             r8, r0, r3, LSL #1  @ sub32(temp, p2)@

    LDRB            r0, [r10], #1       @
    LDRB            r1, [r10], #1       @
    LDRB            r2, [r10], #1       @
    ADD             r0, r11, r0, lsl #3 @
    ADD             r1, r11, r1, lsl #3 @
    ADD             r2, r11, r2, lsl #3 @
    STRD            r4, [r0]            @
    STRD            r6, [r1]            @
    STRD            r8, [r2]            @

    LDMFD           r13!, {r4 - r12, r15}


