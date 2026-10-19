.macro push_v_regs
    stp       q8, q9, [sp, #-32]!
    stp       q10, q11, [sp, #-32]!
    stp       q12, q13, [sp, #-32]!
    stp       q14, q15, [sp, #-32]!
//st1 { v8.2d,  v9.2d, v10.2d, v11.2d}, [sp, #-64]!
//st1 {v12.2d, v13.2d, v14.2d, v15.2d}, [sp, #-64]!
    stp       X8, X9, [sp, #-16]!
    stp       X10, X11, [sp, #-16]!
    stp       X12, X13, [sp, #-16]!
    stp       X14, X15, [sp, #-16]!
  stp       X16, X17, [sp, #-16]!
  stp       X18, X19, [sp, #-16]!
  stp       X20, X21, [sp, #-16]!
  stp       X22, X23, [sp, #-16]!
  stp       X24, X25, [sp, #-16]!
  stp       X26, X27, [sp, #-16]!
  stp       X28, X29, [sp, #-16]!
  stp       X30, X29, [sp, #-16]!
.endm

.macro pop_v_regs
    ldp       X30, X29, [sp], #16
    ldp       X28, X29, [sp], #16
    ldp       X26, X27, [sp], #16
    ldp       X24, X25, [sp], #16
    ldp       X22, X23, [sp], #16
    ldp       X20, X21, [sp], #16
    ldp       X18, X19, [sp], #16
    ldp       X16, X17, [sp], #16
    ldp       X14, X15, [sp], #16
    ldp       X12, X13, [sp], #16
    ldp       X10, X11, [sp], #16
    ldp       X8, X9, [sp], #16
//ld1 {v12.2d, v13.2d, v14.2d, v15.2d}, [sp], #64
//ld1 { v8.2d,  v9.2d, v10.2d, v11.2d}, [sp], #64
    ldp       q14, q15, [sp], #32
    ldp       q12, q13, [sp], #32
    ldp       q10, q11, [sp], #32
    ldp       q8, q9, [sp], #32
.endm


.text
.p2align 2
.global ixheaacd_post_twid_overlap_add_armv8

ixheaacd_post_twid_overlap_add_armv8:

    // STMFD sp!, {x4-x12}
  push_v_regs
  //stp x19, x20,[sp,#-16]!
    //VPUSH           {d8 - d15}

    //LDR w4,  [sp, #100]
  //sxtw x4,w4
    //LDR w5,  [sp, #104]
  //sxtw x5,w5
    //LDR w6,  [sp, #108]
  //sxtw x6,w6
  MOV        x16, x5
  MOV        x17, x7
    LSL             x9, x3, #2
    ASR             x9, x9, #1
    ADD             x6, x6, x9
    SUB             x6, x6, #4

    MOV             w8, #7500
    sxtw            x8, w8
    ADD             x2, x2, x8



    movi  v18.4h, #50
  sub x20, x5, #15
  neg x9, x20
    movi  v20.4s, #0x00, LSL #8
    dup  v16.4s,w5
    SUB             x5, x5, #16
    //STR w5,  [sp, #116]
  MOV        w25, w5
  sxtw x25,w25
    MOV             x8, #1
    LSL             x8, x8, x9
    //STR w8,  [sp, #120]
  MOV        w26, w8

  //sxtw x8,w8


ARM_PROLOGUE:


    LDR w8,  [x1], #4
  sxtw x8,w8
    LDR w9,  [x1], #4
  sxtw x9,w9

    LDR w10,  [x2], #4
  sxtw x10,w10

  AND        w19,w10,0xFFFF
  sxth      x19,w19
  ASR        w10,w10,#16
//    SMULWT          x11, x8, x10
//
//  SMULWB          x12, x9, x10
//    SMULWB          x5, x8, x10
//    SMLAWT          x7, x9, x10, x5

  SMULL          x11, w8, w10
  ASR        x11,x11,#16
  SMULL          x12, w9, w19
  ASR        x12,x12,#16
    SMULL          x5, w8, w19
  ASR        x5,x5,#16
    SMULL      x7, w9, w10
  ASR        x7, x7, #16
  ADD        x7, x7, x5

  SUB             x8, x12, x11
    MVN             x5, x7
    ADD             x5, x5, #1


    MOV             x9, #50
    MOV             x12, #-50
    AND        w19,w9,0xFFFF
  sxth      x19,w19
  SMULL          x10, w5, w19
    ASR        x10,x10,#16
  AND        w19,w12,0xFFFF
  sxth      x19,w19
  SMULL          x11, w8, w19
  ASR        x11,x11,#16

  ADD             x8, x8, x10
    ADD             x5, x5, x11

    //LDR w11,  [sp, #104]
  MOV    w11, w16
  sxth x11,w11
    LDR w10,  [x6], #-32
  sxtw x10,w10

  AND        w19,w10,0xFFFF
  sxth      x19,w19
  ASR        w20,w10,#16

    //SMULWB          x7, x8, x10
    SMULL      x7, w8, w19
  ASR        x7, x7, #16
  MVN             x8, x8
    ADD             x8, x8, #1
    //SMULWT          x12, x8, x10
  SMULL      x12, w8, w20
  ASR        x12, x12, #16

    CMP             x11, #0
    BLT             NEXT

    SUB            x9, x11, #16
  negs      x9,x9




   // LDR w8,  [sp, #120]
  //sxtw x8,w8
  MOV      v1.s[0], w26
  MOV      v2.s[0], w5

    //sQADD            w5, w5, w8
    //ASR             w5, w5, w9

  SQADD      v2.2s, v2.2s, v1.2s
  MOV        w5, v2.s[0]
  ASR             w5, w5, w9

    SUB            x9, x11, #31
  negs      x9,x9
  ASR        x20, x7, x9
    //MOV            x8, x20
  ADDS      x8, x20, #0
  BGE        NEXT2
    CMN           x8, #1
NEXT2:
    MOV             x20, #0x80000000
   csel x7, x20, x7,LT
    MOV             x20, #0x7fffffff
   csel x7, x20, x7,GT
    LSL x20, x7,  x11
  csel x7,x20,x7,EQ

    SUB            x9, x11, #31
  negs      x9,x9
  ASR        x20, x12, x9
    //MOV            x8, x20
  ADDS      x8, x20, #0
  BGE        NEXT3
  CMN           x8, #1
NEXT3:
    MOV             x20, #0x80000000
   csel x12, x20, x12,LT
    MOV             x20, #0x7fffffff
   csel x12, x20, x12,GT
    LSL x20, x12,  x11
  csel x12,x20,x12,EQ

    B               NEXT1
NEXT:
    MVN             w11, w11
    ADD             w11, w11, #1
    ASR             w5, w5, w11
    MOV             w8, #0x8000

  MOV      v1.s[0], w8
  MOV      v2.s[0], w5

    //QADD            x5, x5, x8

  SQADD      v2.2s, v2.2s, v1.2s
  MOV        w5, v2.s[0]

  ASR             w5, w5, #16
    ASR             w7, w7, w11
    ASR             w12, w12, w11

NEXT1:
    LDR w9,  [x4]
  sxtw x9,w9
    MOV             w8, #0x8000
  //sxtw x8,w8

    STR w5,  [x4], #4
  sxtw x5,w5


  ROR        w20, w10, #16
    //UXTH            x5, x10, ROR #16
    UXTH      w5, w20
  UXTH            w10, w10


    dup  v0.2s,w9
    dup  v2.2s,w10
    dup  v3.2s,w5
    //VZIP.32         D2, D3
    ZIP1    v28.2s, v2.2s, v3.2s
  ZIP2    v3.2s, v2.2s, v3.2s
  MOV      v2.8b, v28.8b
  sMULL v0.2d, v2.2s, v0.2s
    Sqxtn v8.2s,  v0.2d


    dup  v0.2s,w12
    dup  v1.2s,w7

    //VZIP.32         D0, D1

    ZIP1    v28.2s, v0.2s, v1.2s
  ZIP2    v1.2s, v0.2s, v1.2s
  MOV      v0.8b, v28.8b

    SQSUB  v8.2s,  v0.2s ,  v8.2s


    //sQshL v8.2s, v8.2s,#2
    dup  v0.2s,w8
    //SQADD  v8.2s,  v8.2s ,  v0.2s
    //sshR v8.2s, v8.2s,#16



    MOV x7,  x17
  //sxtw x7,w7
    LSL             x10, x7, #2

    ASR             x5, x3, #1
    //SMULBB          x5, x10, x5
    AND        w5,w5,0xFFFF
    sxth      x5,w5
  AND        w19,w10,0xFFFF
  sxth      x19,w19
  SMULL      x5, w19, w5

    ADD             x5, x5, x0
    SUB             x0, x5, x10
    MVN             x9, x10
    ADD             x9, x9, #1

    ST1 {V8.S}[1],[x0], x9
    ST1 {V8.S}[0],[x5], x10


    MOV             x8, x1
    LSL             x12, x3, #2

    ADD             x1, x1, x12

    SUB             x1, x1, #40

    MOV             x12, #-32



PROLOGUE_NEON:

    ASR             x3, x3, #2
    SUB             x3, x3, #4
    ASR             x3, x3, #2
    SUB             x3, x3, #2

    LD2 { v0.4s, v1.4s}, [x1]
  MOV        v2.16b, v1.16b
  ADD       x1, x1, x12

    //VUZP.16         D0, D1
    UZP1      v28.8h, v0.8h, v0.8h
  UZP2      v29.8h, v0.8h, v0.8h
  MOV        v0.d[0], v28.d[0]
  MOV        v0.d[1], v29.d[0]

  //VUZP.16         D2, D3

  UZP1      v28.8h, v2.8h, v2.8h
  UZP2      v29.8h, v2.8h, v2.8h
  MOV        v2.d[0], v28.d[0]
  MOV        v2.d[1], v29.d[0]


    //rev64  v0.8h,  v0.8h
  rev64  v0.8h,  v0.8h
  MOV        v1.d[0], v0.d[1]
    rev64  v2.8h,  v2.8h
  MOV        v3.d[0], v2.d[1]
    LD2 {v8.4h, v9.4h}, [x2]
  ADD      x2, x2, #16

    LD2 { v4.4s, v5.4s}, [x8]
  MOV      v6.16b, v5.16b
  ADD x8, x8,#32
    uMULL v30.4s, v0.4h, v9.4h

//    VUZP.16         D4, D5

  UZP1      v28.8h, v4.8h, v4.8h
  UZP2      v29.8h, v4.8h, v4.8h
  MOV        v4.d[0], v28.d[0]
  MOV        v5.d[0], v29.d[0]

    uMULL v28.4s, v2.4h, v8.4h

//    VUZP.16         D6, D7
  UZP1      v26.8h, v6.8h, v6.8h
  UZP2      v27.8h, v6.8h, v6.8h
  MOV        v6.d[0], v26.d[0]
  MOV        v7.d[0], v27.d[0]

  uMULL v26.4s, v0.4h, v8.4h


    uMULL v24.4s, v2.4h, v9.4h

    LD2 { v10.4s, v11.4s}, [x6]
    MOV      v12.16b, v11.16b
  ADD  x6, x6, x12
    ushR v30.4s, v30.4s,#16

    //VUZP.16         D10, D11

  UZP1      v22.8h, v10.8h, v10.8h
  UZP2      v23.8h, v10.8h, v10.8h
  MOV        v10.d[0], v22.d[0]
  MOV        v10.d[1], v23.d[0]

  ushR v28.4s, v28.4s,#16

    //VUZP.16         D12, D13

  UZP1      v22.8h, v12.8h, v12.8h
  UZP2      v23.8h, v12.8h, v12.8h
  MOV        v12.d[0], v22.d[0]
  MOV        v12.d[1], v23.d[0]

  sMLAL v30.4s, v1.4h, v9.4h

    rev64  v10.8h,  v10.8h
  MOV    v11.d[0], v10.d[1]
    sMLAL v28.4s, v3.4h, v8.4h

    rev64  v12.8h,  v12.8h
  MOV    v13.d[0], v12.d[1]
    ushR v26.4s, v26.4s,#16


    ushR v24.4s, v24.4s,#16

    sMLAL v26.4s, v1.4h, v8.4h
    sMLAL v24.4s, v3.4h, v9.4h



    ADD  v30.4s,  v30.4s ,  v28.4s
    NEG  v30.4s, v30.4s

    uMULL v22.4s, v4.4h, v8.4h

    SUB  v28.4s,  v24.4s ,  v26.4s


    mov  v26.16b, v30.16b
    mov  v24.16b, v28.16b

//    VUZP.16         D24, D25

  UZP1      v19.8h, v24.8h, v24.8h
  UZP2      v21.8h, v24.8h, v24.8h
  MOV        v24.d[0], v19.d[0]
  MOV        v25.d[0], v21.d[0]


//    VUZP.16         D26, D27

  UZP1      v19.8h, v26.8h, v26.8h
  UZP2      v21.8h, v26.8h, v26.8h
  MOV        v26.d[0], v19.d[0]
  MOV        v27.d[0], v21.d[0]

    uMULL v2.4s, v24.4h, v18.4h

    uMULL v0.4s, v26.4h, v18.4h

    ushR v22.4s, v22.4s,#16
    sMLAL v22.4s, v5.4h, v8.4h

    ushR v2.4s, v2.4s,#16
    ushR v0.4s, v0.4s,#16
    sMLAL v2.4s, v25.4h, v18.4h
    sMLAL v0.4s, v27.4h, v18.4h

    uMULL v24.4s, v4.4h, v9.4h
    uMULL v26.4s, v6.4h, v8.4h

    NEG  v2.4s, v2.4s
    ADD  v28.4s,  v28.4s ,  v0.4s
    ADD  v30.4s,  v30.4s ,  v2.4s

    uMULL v0.4s, v6.4h, v9.4h
    sshR v24.4s, v24.4s,#16
    sMLAL v24.4s, v5.4h, v9.4h
    sshR v26.4s, v26.4s,#16
    sshR v0.4s, v0.4s,#16
    sMLAL v26.4s, v7.4h, v8.4h
    sMLAL v0.4s, v7.4h, v9.4h




    ADD  v22.4s,  v22.4s ,  v0.4s
    NEG  v22.4s, v22.4s
    SUB  v24.4s,  v26.4s ,  v24.4s



    //LDR w11,  [sp, #120]
  //sxtw x11,w11
  MOV      w11, w26
    dup  v14.4s,w11
    SQADD  v28.4s,  v28.4s ,  v14.4s
    //LDR w11,  [sp, #116]
  MOV    w11, w25
  //sxtw x11,w11
    dup  v0.4s,w11
    sQshL v28.4s, v28.4s, v0.4s

    mov  v0.16b, v22.16b
    mov  v14.16b, v24.16b


//    VUZP.16         D24, D25

  UZP1      v19.8h, v24.8h, v24.8h
  UZP2      v21.8h, v24.8h, v24.8h
  MOV        v24.d[0], v19.d[0]
  MOV        v25.d[0], v21.d[0]


//    VUZP.16         D22, D23

  UZP1      v19.8h, v22.8h, v22.8h
  UZP2      v21.8h, v22.8h, v22.8h
  MOV        v22.d[0], v19.d[0]
  MOV        v23.d[0], v21.d[0]

  uMULL v8.4s, v24.4h, v18.4h
    uMULL v26.4s, v22.4h, v18.4h

    NEG  v2.4s, v30.4s
//    VUZP.16         D30, D31

  UZP1      v19.8h, v30.8h, v30.8h
  UZP2      v21.8h, v30.8h, v30.8h
  MOV        v30.d[0], v19.d[0]
  MOV        v30.d[1], v21.d[0]

//    VUZP.16         D2, D3

  UZP1      v19.8h, v2.8h, v2.8h
  UZP2      v21.8h, v2.8h, v2.8h
  MOV        v2.d[0], v19.d[0]
  MOV        v3.d[0], v21.d[0]

    uMULL v4.4s, v30.4h, v12.4h

    uMULL v6.4s, v2.4h, v13.4h

    ushR v8.4s, v8.4s,#16
    ushR v26.4s, v26.4s,#16

    sMLAL v8.4s, v25.4h, v18.4h
    sMLAL v26.4s, v23.4h, v18.4h

    ushR v4.4s, v4.4s,#16
    ushR v6.4s, v6.4s,#16

  MOV  v19.d[0], v30.d[1]

    sMLAL v4.4s, v19.4h, v12.4h
    sMLAL v6.4s, v3.4h, v13.4h

    NEG  v8.4s, v8.4s
    ADD  v14.4s,  v14.4s ,  v26.4s
    ADD  v0.4s,  v0.4s ,  v8.4s

    //LDR w11,  [sp, #120]
  //sxtw x11,w11
  MOV      w11, w26
    dup  v8.4s,w11
    SQADD  v0.4s,  v0.4s ,  v8.4s
    //LDR w11,  [sp, #116]
  //sxtw x11,w11
    MOV    w11, w25
  dup  v26.4s,w11
    sQshL v0.4s, v0.4s, v26.4s

    mov  v26.16b, v28.16b

    LD2 { v28.4s, v29.4s}, [x4]
    MOV      v30.16b, v29.16b
    MOV      v29.d[0], v28.d[1]
 //   VZIP.32         Q13, Q0

    ZIP1    v19.4s, v26.4s, v0.4s
  ZIP2    v0.4s, v26.4s, v0.4s
  MOV      v26.16b, v19.16b

    ST1 { v26.4s}, [x4],#16
    ST1 { v0.4s}, [x4],#16

    movi  v1.2s, #0
    //VADDL.S16       Q0, D13, D1

  SADDL       v0.4s, v13.4h, v1.4h
  MOV      v1.d[0], v0.d[1]
    sMULL v26.2d, v28.2s, v0.2s
    Sqxtn v8.2s,  v26.2d
    sMULL v26.2d, v29.2s, v1.2s
    Sqxtn v9.2s,  v26.2d
  MOV    v8.d[1], v9.d[0]
    movi  v1.2s, #0
//    VADDL.S16       Q0, D12, D1
  SADDL       v0.4s, v12.4h, v1.4h
  MOV      v1.d[0], v0.d[1]
    sMULL v24.2d, v28.2s, v0.2s
    Sqxtn v26.2s,  v24.2d
    sMULL v24.2d, v29.2s, v1.2s
    Sqxtn v27.2s,  v24.2d
  MOV    v26.d[1], v27.d[0]

    sQshL v4.4s, v4.4s, v16.4s
    sQshL v6.4s, v6.4s, v16.4s

    SQSUB  v4.4s,  v4.4s ,  v8.4s
    SQSUB  v6.4s,  v6.4s ,  v26.4s

    NEG  v26.4s, v14.4s
//    VUZP.16         D14, D15


  UZP1      v19.8h, v14.8h, v14.8h
  UZP2      v21.8h, v14.8h, v14.8h
  MOV        v14.d[0], v19.d[0]
  MOV        v15.d[0], v21.d[0]

//    VUZP.16         D26, D27


  UZP1      v19.8h, v26.8h, v26.8h
  UZP2      v21.8h, v26.8h, v26.8h
  MOV        v26.d[0], v19.d[0]
  MOV        v27.d[0], v21.d[0]


    movi  v1.2s, #0
//    VADDL.S16       Q0, D10, D1
   SADDL       v0.4s, v10.4h, v1.4h
  MOV      v1.d[0], v0.d[0]
  sMULL v22.2d, v30.2s, v0.2s
    Sqxtn v24.2s,  v22.2d
    sMULL2 v22.2d, v30.4s, v0.4s
    Sqxtn v25.2s,  v22.2d
  MOV    v24.d[1], v25.d[0]
    movi  v1.2s, #0
//    VADDL.S16       Q0, D11, D1
  SADDL       v0.4s, v11.4h, v1.4h
  MOV      v1.d[0], v0.d[1]

  sMULL v8.2d, v30.2s, v0.2s
    Sqxtn v22.2s,  v8.2d
    sMULL2 v8.2d, v30.4s, v0.4s
    Sqxtn v23.2s,  v8.2d
  MOV    v22.d[1], v23.d[0]
    uMULL v8.4s, v26.4h, v11.4h
    uMULL v30.4s, v14.4h, v10.4h

    LD2 { v0.4s, v1.4s}, [x1]
  MOV  v2.16b, v1.16b
  ADD x1, x1, x12

//    VUZP.16         D0, D1

  UZP1      v19.8h, v0.8h, v0.8h
  UZP2      v21.8h, v0.8h, v0.8h
  MOV        v0.d[0], v19.d[0]
  MOV        v0.d[1], v21.d[0]

//    VUZP.16         D2, D3

  UZP1      v19.8h, v2.8h, v2.8h
  UZP2      v21.8h, v2.8h, v2.8h
  MOV        v2.d[0], v19.d[0]
  MOV        v2.d[1], v21.d[0]

    ushR v8.4s, v8.4s,#16

    rev64  v0.8h,  v0.8h
  MOV    v1.d[0], v0.d[1]
    ushR v30.4s, v30.4s,#16

    rev64  v2.8h,  v2.8h
  MOV    v3.d[0], v2.d[1]
    sMLAL v8.4s, v27.4h, v11.4h

    sMLAL v30.4s, v15.4h, v10.4h

    LD2 { v10.4s, v11.4s}, [x6]
  ADD  x6, x6, x12
  MOV  v12.16b, v11.16b

  UZP1      v19.8h, v10.8h, v10.8h
  UZP2      v21.8h, v10.8h, v10.8h
  MOV        v10.d[0], v19.d[0]
  MOV        v10.d[1], v21.d[0]


 UZP1   v19.8h, v12.8h, v12.8h
 UZP2   v21.8h, v12.8h, v12.8h
 MOV    v12.d[0], v19.d[0]
 MOV    v12.d[1], v21.d[0]
  MOV V14.16B , V4.16B

    rev64  v10.8h,  v10.8h
  MOV    v11.d[0], v10.d[1]


    rev64  v12.8h,  v12.8h
  MOV   v13.d[0], v12.d[1]

    sQshL v8.4s, v8.4s, v16.4s

    MOV V31.16B, V6.16B
    LD2 { v4.4s, v5.4s}, [x8]
  ADD  x8, x8,#32

  MOV  v6.16b, v5.16b
    sQshL v30.4s, v30.4s, v16.4s

//    VUZP.16         D4, D5

  UZP1      v19.8h, v4.8h, v4.8h
  UZP2      v21.8h, v4.8h, v4.8h
  MOV        v4.d[0], v19.d[0]
  MOV        v5.d[0], v21.d[0]

    SQSUB  v8.4s,  v8.4s ,  v24.4s

//    VUZP.16         D6, D7

    UZP1      v19.8h, v6.8h, v6.8h
  UZP2      v21.8h, v6.8h, v6.8h
  MOV        v6.d[0], v19.d[0]
  MOV        v7.d[0], v21.d[0]

  SQSUB  v22.4s,  v30.4s ,  v22.4s


     MOV V30.16B, V8.16B

    LD2 {v8.4h, v9.4h}, [x2]
  ADD  x2, x2, #16


CORE_LOOP:
    ST1 {V14.S}[0], [x0]
  ADD  x0, x0, x9
    ST1 {V22.S}[0], [x0]
  ADD  x0, x0,  x9


    ST1 {V14.S}[1], [x0]
  ADD  x0, x0, x9


    ST1 {V22.S}[1], [x0]
  ADD  x0, x0,  x9


    ST1 {V14.S}[2], [x0]
    ADD  x0, x0,  x9


    ST1 {V22.S}[2], [x0]
    ADD  x0, x0,  x9


    ST1 {V14.S}[3], [x0]
    ADD  x0, x0,  x9


    ST1 {V22.S}[3], [x0]
    ADD  x0, x0,  x9


    ST1 {V31.S}[0], [x5]
    ADD  x5, x5,  x10


    ST1 {V30.S}[0], [x5]
  ADD  x5, x5,  x10


    ST1 {V31.S}[1], [x5]
  ADD  x5, x5,  x10


    ST1 {V30.S}[1], [x5]
  ADD  x5, x5,  x10


    ST1 {V31.S}[2], [x5]
  ADD  x5, x5,  x10


    ST1 {V30.S}[2], [x5]
  ADD  x5, x5,  x10


    ST1 {V31.S}[3], [x5]
  ADD  x5, x5,  x10

    ST1 {V30.S}[3], [x5]
  ADD  x5, x5,  x10


    uMULL v30.4s, v0.4h, v9.4h
    uMULL v28.4s, v2.4h, v8.4h
    uMULL v26.4s, v0.4h, v8.4h
    uMULL v24.4s, v2.4h, v9.4h
    ushR v30.4s, v30.4s,#16
    ushR v28.4s, v28.4s,#16
    sMLAL v30.4s, v1.4h, v9.4h
    sMLAL v28.4s, v3.4h, v8.4h
    ushR v26.4s, v26.4s,#16
    ushR v24.4s, v24.4s,#16
    sMLAL v26.4s, v1.4h, v8.4h
    sMLAL v24.4s, v3.4h, v9.4h
    ADD  v30.4s,  v30.4s ,  v28.4s
    NEG  v30.4s, v30.4s
    SUB  v28.4s,  v24.4s ,  v26.4s

    mov  v26.16b, v30.16b
    uMULL v22.4s, v4.4h, v8.4h

    mov  v24.16b, v28.16b

//    VUZP.16         D24, D25

  UZP1      v19.8h, v24.8h, v24.8h
  UZP2      v21.8h, v24.8h, v24.8h
  MOV        v24.d[0], v19.d[0]
  MOV        v25.d[0], v21.d[0]


//    VUZP.16         D26, D27

  UZP1      v19.8h, v26.8h, v26.8h
  UZP2      v21.8h, v26.8h, v26.8h
  MOV        v26.d[0], v19.d[0]
  MOV        v27.d[0], v21.d[0]

  uMULL v2.4s, v24.4h, v18.4h
    uMULL v0.4s, v26.4h, v18.4h

    ushR v22.4s, v22.4s,#16
    sMLAL v22.4s, v5.4h, v8.4h

    ushR v2.4s, v2.4s,#16
    ushR v0.4s, v0.4s,#16
    sMLAL v2.4s, v25.4h, v18.4h
    sMLAL v0.4s, v27.4h, v18.4h

    uMULL v24.4s, v4.4h, v9.4h
    uMULL v26.4s, v6.4h, v8.4h

    NEG  v2.4s, v2.4s
    ADD  v28.4s,  v28.4s ,  v0.4s
    ADD  v30.4s,  v30.4s ,  v2.4s

    uMULL v0.4s, v6.4h, v9.4h
    sshR v24.4s, v24.4s,#16
    sMLAL v24.4s, v5.4h, v9.4h
    sshR v26.4s, v26.4s,#16
    sshR v0.4s, v0.4s,#16
    sMLAL v26.4s, v7.4h, v8.4h
    sMLAL v0.4s, v7.4h, v9.4h



    ADD  v22.4s,  v22.4s ,  v0.4s

    NEG  v22.4s, v22.4s
    SUB  v24.4s,  v26.4s ,  v24.4s


    //LDR w11,  [sp, #120]
  //sxtw x11,w11
  MOV      w11, w26
    dup  v14.4s,w11
    SQADD  v28.4s,  v28.4s ,  v14.4s
    //LDR w11,  [sp, #116]
  //sxtw x11,w11
  MOV    w11, w25
    dup  v0.4s,w11
    sQshL v28.4s, v28.4s, v0.4s


    mov  v0.16b, v22.16b
    mov  v14.16b, v24.16b

//    VUZP.16         D24, D25

  UZP1      v19.8h, v24.8h, v24.8h
  UZP2      v21.8h, v24.8h, v24.8h
  MOV        v24.d[0], v19.d[0]
  MOV        v25.d[0], v21.d[0]


//    VUZP.16         D22, D23

  UZP1      v19.8h, v22.8h, v22.8h
  UZP2      v21.8h, v22.8h, v22.8h
  MOV        v22.d[0], v19.d[0]
  MOV        v23.d[0], v21.d[0]

  uMULL v8.4s, v24.4h, v18.4h
    uMULL v26.4s, v22.4h, v18.4h

    NEG  v2.4s, v30.4s

//    VUZP.16         D30, D31

  UZP1      v19.8h, v30.8h, v30.8h
  UZP2      v21.8h, v30.8h, v30.8h
  MOV        v30.d[0], v19.d[0]
  MOV        v30.d[1], v21.d[0]


//    VUZP.16         D2, D3

  UZP1      v19.8h, v2.8h, v2.8h
  UZP2      v21.8h, v2.8h, v2.8h
  MOV        v2.d[0], v19.d[0]
  MOV        v3.d[0], v21.d[0]

  uMULL v4.4s, v30.4h, v12.4h
    uMULL v6.4s, v2.4h, v13.4h

    ushR v8.4s, v8.4s,#16
    ushR v26.4s, v26.4s,#16

    sMLAL v8.4s, v25.4h, v18.4h
    sMLAL v26.4s, v23.4h, v18.4h

    ushR v4.4s, v4.4s,#16
    ushR v6.4s, v6.4s,#16

  MOV  v19.d[0], v30.d[1]

    sMLAL v4.4s, v19.4h, v12.4h
    sMLAL v6.4s, v3.4h, v13.4h

    NEG  v8.4s, v8.4s
    ADD  v14.4s,  v14.4s ,  v26.4s
    ADD  v0.4s,  v0.4s ,  v8.4s



    //LDR w11,  [sp, #120]
  //sxtw x11,w11
  MOV      w11, w26
    dup  v8.4s,w11
    SQADD  v0.4s,  v0.4s ,  v8.4s
    //LDR w11,  [sp, #116]
  //sxtw x11,w11
    MOV    w11, w25
  dup  v26.4s,w11
    sQshL v0.4s, v0.4s, v26.4s
    mov  v26.16b, v28.16b

    LD2 { v28.4s, v29.4s}, [x4]
  MOV  v30.16b, v29.16b
  MOV  v29.d[0], v28.d[1]
//    VZIP.32         Q13, Q0

    ZIP1    v19.4s, v26.4s, v0.4s
  ZIP2    v0.4s, v26.4s, v0.4s
  MOV      v26.16b, v19.16b

    ST1 { v26.4s}, [x4]
  ADD x4, x4,#16
    ST1 { v0.4s}, [x4]
  ADD  x4, x4,#16

    movi  v1.2s, #0
//    VADDL.S16       Q0, D13, D1
  SADDL       v0.4s, v13.4h, v1.4h
  MOV      v1.d[0], v0.d[1]

    sMULL v26.2d, v28.2s, v0.2s
    Sqxtn v8.2s,  v26.2d
    sMULL v26.2d, v29.2s, v1.2s
    Sqxtn v9.2s,  v26.2d
  MOV    v8.d[1], v9.d[0]
    movi  v1.2s, #0
    //VADDL.S16       Q0, D12, D1
  SADDL       v0.4s, v12.4h, v1.4h
  MOV      v1.d[0], v0.d[1]

    sMULL v24.2d, v28.2s, v0.2s
    Sqxtn v26.2s,  v24.2d
    sMULL v24.2d, v29.2s, v1.2s
    Sqxtn v27.2s,  v24.2d
  MOV    v26.d[1], v27.d[0]
    sQshL v4.4s, v4.4s, v16.4s
    sQshL v6.4s, v6.4s, v16.4s



    SQSUB  v4.4s,  v4.4s ,  v8.4s
    SQSUB  v6.4s,  v6.4s ,  v26.4s

    NEG  v26.4s, v14.4s
//    VUZP.16         D26, D27
  UZP1      v19.8h, v26.8h, v26.8h
  UZP2      v21.8h, v26.8h, v26.8h
  MOV        v26.d[0], v19.d[0]
  MOV        v27.d[0], v21.d[0]

    movi  v1.2s, #0
   //VADDL.S16       Q0, D10, D1
      SADDL       v0.4s, v10.4h, v1.4h
  MOV      v1.d[0], v0.d[1]

  sMULL v22.2d, v30.2s, v0.2s
    Sqxtn v24.2s,  v22.2d
    sMULL2 v22.2d, v30.4s, v0.4s
    Sqxtn v25.2s,  v22.2d
  MOV    v24.d[1], v25.d[0]
    movi  v1.2s, #0
    //VADDL.S16       Q0, D11, D1
  SADDL       v0.4s, v11.4h, v1.4h

    sMULL v8.2d, v30.2s, v0.2s
    Sqxtn v22.2s,  v8.2d
    sMULL2 v8.2d, v30.4s, v0.4s
    Sqxtn v23.2s,  v8.2d
  MOV    v22.d[1], v23.d[0]

//    VUZP.16         D14, D15

   UZP1      v19.8h, v14.8h, v14.8h
  UZP2      v21.8h, v14.8h, v14.8h
  MOV        v14.d[0], v19.d[0]
  MOV        v15.d[0], v21.d[0]

  uMULL v8.4s, v26.4h, v11.4h
    uMULL v30.4s, v14.4h, v10.4h


    LD2 { v0.4s, v1.4s}, [x1]
  MOV v2.16b, v1.16b
  ADD  X1, X1, x12

//    VUZP.16         D0, D1
  UZP1      v19.8h, v0.8h, v0.8h
  UZP2      v21.8h, v0.8h, v0.8h
  MOV        v0.d[0], v19.d[0]
  MOV        v0.d[1], v21.d[0]

//    VUZP.16         D2, D3

   UZP1      v19.8h, v2.8h, v2.8h
  UZP2      v21.8h, v2.8h, v2.8h
  MOV        v2.d[0], v19.d[0]
  MOV        v2.d[1], v21.d[0]

  ushR v8.4s, v8.4s,#16

    rev64  v0.8h,  v0.8h
  MOV  v1.d[0], v0.d[1]
    ushR v30.4s, v30.4s,#16

    rev64  v2.8h,  v2.8h
  MOV  v3.d[0], v2.d[1]
    sMLAL v8.4s, v27.4h, v11.4h

    sMLAL v30.4s, v15.4h, v10.4h

    LD2 { v10.4s, v11.4s}, [x6]
  add  X6, x6, x12
  MOV  v12.16b, v11.16b


    //VUZP.16         D10, D11

     UZP1      v19.8h, v10.8h, v10.8h
  UZP2      v21.8h, v10.8h, v10.8h
  MOV        v10.d[0], v19.d[0]
  MOV        v10.d[1], v21.d[0]



//    VUZP.16         D12, D13

   UZP1      v19.8h, v12.8h, v12.8h
  UZP2      v21.8h, v12.8h, v12.8h
        MOV        v12.d[0], v19.d[0]
  MOV        v12.d[1], v21.d[0]



      MOV  V14.16B, V4.16B

    rev64  v10.8h,  v10.8h
  MOV        v11.d[0], v10.d[1]


    rev64  v12.8h,  v12.8h
  MOV        v13.d[0], v12.d[1]

    sQshL v8.4s, v8.4s, v16.4s

    LD2 { v4.4s, v5.4s}, [x8]
  ADD  x8, x8, #32

    MOV V31.16B, V6.16B
    MOV  v6.16b, v5.16b

  sQshL v30.4s, v30.4s, v16.4s


   UZP1      v19.8h, v4.8h, v4.8h
  UZP2      v21.8h, v4.8h, v4.8h
  MOV        v4.d[0], v19.d[0]
  MOV        v5.d[0], v21.d[0]


  SQSUB  v8.4s,  v8.4s ,  v24.4s

//    VUZP.16         D6, D7

     UZP1      v19.8h, v6.8h, v6.8h
  UZP2      v21.8h, v6.8h, v6.8h
  MOV        v6.d[0], v19.d[0]
  MOV        v7.d[0], v21.d[0]

  SQSUB  v22.4s,  v30.4s ,  v22.4s

    MOV V30.16B , V8.16B

    LD2 {v8.4h, v9.4h}, [x2]
  ADD x2, x2,#16




    SUBS            x3, x3, #1
    BNE             CORE_LOOP





EPILOGUE:

    ST1 {V14.S}[0],[x0]
  ADD  x0, x0, x9


    ST1 {V22.S}[0],[x0]
  ADD  x0, x0, x9


    ST1 {V14.S}[1],[x0]
  ADD  x0, x0, x9


    ST1 {V22.S}[1],[x0]
  ADD  x0, x0, x9


    ST1 {V14.S}[2],[x0]
  ADD  x0, x0, x9


    ST1 {V22.S}[2],[x0]
  ADD  x0, x0, x9


    ST1 {V14.S}[3],[x0]
  ADD  x0, x0, x9


    ST1 {V22.S}[3],[x0]
  ADD  x0, x0, x9


    ST1 {V31.S}[0],[x5]
  ADD  x5, x5, x10


    ST1 {V30.S}[0],[x5]
  ADD  x5, x5, x10


    ST1 {V31.S}[1],[x5]
  ADD  x5, x5, x10


    ST1 {V30.S}[1],[x5]
  ADD  x5, x5, x10


    ST1 {V31.S}[2],[x5]
  ADD  x5, x5, x10


    ST1 {V30.S}[2],[x5]
  ADD  x5, x5, x10


    ST1 {V31.S}[3],[x5]
  ADD  x5, x5, x10


    ST1 {V30.S}[3],[x5]
  ADD  x5, x5, x10


    uMULL v30.4s, v0.4h, v9.4h
    uMULL v28.4s, v2.4h, v8.4h
    uMULL v26.4s, v0.4h, v8.4h
    uMULL v24.4s, v2.4h, v9.4h
    ushR v30.4s, v30.4s,#16
    ushR v28.4s, v28.4s,#16
    sMLAL v30.4s, v1.4h, v9.4h
    sMLAL v28.4s, v3.4h, v8.4h
    ushR v26.4s, v26.4s,#16
    ushR v24.4s, v24.4s,#16
    sMLAL v26.4s, v1.4h, v8.4h
    sMLAL v24.4s, v3.4h, v9.4h
    ADD  v30.4s,  v30.4s ,  v28.4s
    NEG  v30.4s, v30.4s
    SUB  v28.4s,  v24.4s ,  v26.4s


    uMULL v22.4s, v4.4h, v8.4h
    mov  v26.16b, v30.16b
    mov  v24.16b, v28.16b

    mov  v26.16b, v30.16b
    mov  v24.16b, v28.16b

    //VUZP.16         D26, D27

  UZP1      v19.8h, v26.8h, v26.8h
  UZP2      v21.8h, v26.8h, v26.8h
  MOV        v26.d[0], v19.d[0]
  MOV        v27.d[0], v21.d[0]

//    VUZP.16         D24, D25

     UZP1      v19.8h, v24.8h, v24.8h
  UZP2      v21.8h, v24.8h, v24.8h
  MOV        v24.d[0], v19.d[0]
  MOV        v25.d[0], v21.d[0]

    uMULL v2.4s, v24.4h, v18.4h
    uMULL v0.4s, v26.4h, v18.4h

    ushR v22.4s, v22.4s,#16
    sMLAL v22.4s, v5.4h, v8.4h

    ushR v2.4s, v2.4s,#16
    ushR v0.4s, v0.4s,#16
    sMLAL v2.4s, v25.4h, v18.4h
    sMLAL v0.4s, v27.4h, v18.4h

    uMULL v24.4s, v4.4h, v9.4h
    uMULL v26.4s, v6.4h, v8.4h

    NEG  v2.4s, v2.4s
    ADD  v28.4s,  v28.4s ,  v0.4s
    ADD  v30.4s,  v30.4s ,  v2.4s

    uMULL v0.4s, v6.4h, v9.4h
    sshR v24.4s, v24.4s,#16
    sMLAL v24.4s, v5.4h, v9.4h
    sshR v26.4s, v26.4s,#16
    sshR v0.4s, v0.4s,#16
    sMLAL v26.4s, v7.4h, v8.4h
    sMLAL v0.4s, v7.4h, v9.4h





    ADD  v22.4s,  v22.4s ,  v0.4s
    NEG  v22.4s, v22.4s
    SUB  v24.4s,  v26.4s ,  v24.4s




    //LDR w11,  [sp, #120]
  //sxtw x11,w11
  MOV      w11, w26
    dup  v14.4s,w11
    SQADD  v28.4s,  v28.4s ,  v14.4s
    //LDR w11,  [sp, #116]
  //sxtw x11,w11
    MOV    w11, w25
  dup  v0.4s,w11
    sQshL v28.4s, v28.4s, v0.4s


    mov  v0.16b, v22.16b
    mov  v14.16b, v24.16b


//    VUZP.16         D22, D23

  UZP1      v19.8h, v22.8h, v22.8h
  UZP2      v21.8h, v22.8h, v22.8h
  MOV        v22.d[0], v19.d[0]
  MOV        v23.d[0], v21.d[0]

//    VUZP.16         D24, D25

     UZP1      v19.8h, v24.8h, v24.8h
  UZP2      v21.8h, v24.8h, v24.8h
  MOV        v24.d[0], v19.d[0]
  MOV        v25.d[0], v21.d[0]

    uMULL v8.4s, v24.4h, v18.4h
    uMULL v26.4s, v22.4h, v18.4h

    NEG  v2.4s, v30.4s

//    VUZP.16         D30, D31

  UZP1      v19.8h, v30.8h, v30.8h
  UZP2      v21.8h, v30.8h, v30.8h
  MOV        v30.d[0], v19.d[0]
  MOV        v30.d[1], v21.d[0]

//    VUZP.16         D2, D3

   UZP1      v19.8h, v2.8h, v2.8h
  UZP2      v21.8h, v2.8h, v2.8h
  MOV        v2.d[0], v19.d[0]
  MOV        v3.d[0], v21.d[0]

    uMULL v4.4s, v30.4h, v12.4h
    uMULL v6.4s, v2.4h, v13.4h

    ushR v8.4s, v8.4s,#16
    ushR v26.4s, v26.4s,#16

    sMLAL v8.4s, v25.4h, v18.4h
    sMLAL v26.4s, v23.4h, v18.4h

    ushR v4.4s, v4.4s,#16
    ushR v6.4s, v6.4s,#16

  MOV  v19.d[0], v30.d[1]

    sMLAL v4.4s, v19.4h, v12.4h
    sMLAL v6.4s, v3.4h, v13.4h

    NEG  v8.4s, v8.4s
    ADD  v14.4s,  v14.4s ,  v26.4s
    ADD  v0.4s,  v0.4s ,  v8.4s

    //LDR w11,  [sp, #120]
  //sxtw x11,w11
  MOV      w11, w26
    dup  v8.4s,w11
    SQADD  v0.4s,  v0.4s ,  v8.4s
    //LDR w11,  [sp, #116]
  //sxtw x11,w11
    MOV    w11, w25
  dup  v26.4s,w11
    sQshL v0.4s, v0.4s, v26.4s


    mov  v26.16b, v28.16b

    LD2 { v28.4s, v29.4s}, [x4]
  MOV  v30.16b, v29.16b
  MOV  v29.d[0], v28.d[1]
//    VZIP.32         Q13, Q0

    ZIP1    v19.4s, v26.4s, v0.4s
  ZIP2    v0.4s, v26.4s, v0.4s
  MOV      v26.16b, v19.16b

  ST1 { v26.4s}, [x4],#16
    ST1 { v0.4s}, [x4],#16

    movi  v1.2s, #0
//    VADDL.S16       Q0, D13, D1
  SADDL       v0.4s, v13.4h, v1.4h
  MOV      v1.d[0], v0.d[1]

    sMULL v26.2d, v28.2s, v0.2s
    Sqxtn v8.2s,  v26.2d
    sMULL v26.2d, v29.2s, v1.2s
    Sqxtn v9.2s,  v26.2d
  MOV    v8.d[1], v9.d[0]
    movi  v1.2s, #0
//    VADDL.S16       Q0, D12, D1
  SADDL       v0.4s, v12.4h, v1.4h
  MOV      v1.d[0], v0.d[1]

    sMULL v24.2d, v28.2s, v0.2s
    Sqxtn v26.2s,  v24.2d
    sMULL v24.2d, v29.2s, v1.2s
    Sqxtn v27.2s,  v24.2d
  MOV    v26.d[1], v27.d[0]

    sQshL v4.4s, v4.4s, v16.4s
    sQshL v6.4s, v6.4s, v16.4s

    SQSUB  v4.4s,  v4.4s ,  v8.4s
    SQSUB  v6.4s,  v6.4s ,  v26.4s

    NEG  v26.4s, v14.4s
//    VUZP.16         D14, D15

  UZP1      v19.8h, v14.8h, v14.8h
  UZP2      v21.8h, v14.8h, v14.8h
  MOV        v14.d[0], v19.d[0]
  MOV        v15.d[0], v21.d[0]


 //   VUZP.16         D26, D27

  UZP1      v19.8h, v26.8h, v26.8h
  UZP2      v21.8h, v26.8h, v26.8h
  MOV        v26.d[0], v19.d[0]
  MOV        v27.d[0], v21.d[0]


    movi  v1.2s, #0
    //VADDL.S16       Q0, D10, D1
  SADDL       v0.4s, v10.4h, v1.4h
  MOV      v1.d[0], v0.d[1]

    sMULL v22.2d, v30.2s, v0.2s
    Sqxtn v24.2s,  v22.2d
    sMULL2 v22.2d, v30.4s, v0.4s
    Sqxtn v25.2s,  v22.2d
  MOV    v24.d[1], v25.d[0]
    movi  v1.2s, #0
    //VADDL.S16       Q0, D11, D1
  SADDL       v0.4s, v11.4h, v1.4h
  MOV      v1.d[0], v0.d[1]

    sMULL v8.2d, v30.2s, v0.2s
    Sqxtn v22.2s,  v8.2d
    sMULL2 v8.2d, v30.4s, v0.4s
    Sqxtn v23.2s,  v8.2d
  MOV    v22.d[1], v23.d[0]

    uMULL v8.4s, v26.4h, v11.4h
    uMULL v30.4s, v14.4h, v10.4h

    ushR v8.4s, v8.4s,#16

    ushR v30.4s, v30.4s,#16

    sMLAL v8.4s, v27.4h, v11.4h

    sMLAL v30.4s, v15.4h, v10.4h


    MOV V14.16B, V4.16B


    sQshL v8.4s, v8.4s, v16.4s

    sQshL v30.4s, v30.4s, v16.4s

    SQSUB  v8.4s,  v8.4s ,  v24.4s

    SQSUB  v22.4s,  v30.4s ,  v22.4s

    MOV V30.16B , V8.16B




    ST1 {V14.S}[0],[x0]
  ADD  x0, x0, x9
    ST1 {V22.S}[0],[x0]
       ADD  x0, x0, x9
    ST1 {V14.S}[1],[x0]
  ADD  x0, x0, x9
    ST1 {V22.S}[1],[x0]
  ADD  x0, x0, x9
    ST1 {V14.S}[2],[x0]
  ADD  x0, x0, x9
    ST1 {V22.S}[2],[x0]
  ADD  x0, x0, x9
    ST1 {V14.S}[3],[x0]
  ADD  x0, x0, x9
    ST1 {V22.S}[3],[x0]
  ADD  x0, x0, x9
    ST1 {V6.S}[0],[x5]
  ADD  x5, x5, x10
    ST1 {V30.S}[0],[x5]
  ADD  x5, x5, x10
    ST1 {V6.S}[1],[x5]
  ADD  x5, x5, x10
    ST1 {V30.S}[1],[x5]
  ADD  x5, x5, x10
    ST1 {V6.S}[2],[x5]
  ADD  x5, x5, x10
    ST1 {V30.S}[2],[x5]
  ADD  x5, x5, x10
    ST1 {V6.S}[3],[x5]
  ADD  x5, x5, x10
    ST1 {V30.S}[3],[x5]
  ADD  x5, x5, x10

ARM_EPILOGUE:

ARM_LOOP:

    LD2 { v0.4s, v1.4s}, [x1]
  MOV  v2.16b, v1.16b

    //VUZP.16         D0, D1
      UZP1      v19.8h, v0.8h, v0.8h
  UZP2      v21.8h, v0.8h, v0.8h
  MOV        v0.d[0], v19.d[0]
  MOV        v0.d[1], v21.d[0]

  //VUZP.16         D2, D3
  UZP1      v19.8h, v2.8h, v2.8h
  UZP2      v21.8h, v2.8h, v2.8h
  MOV        v2.d[0], v19.d[0]
  MOV        v2.d[1], v21.d[0]


    rev64  v0.8h,  v0.8h
  MOV        v1.d[0], v0.d[1]
    rev64  v2.8h,  v2.8h
  MOV        v3.d[0], v2.d[1]

    LD2 {v8.4h, v9.4h}, [x2]
  ADD x2, x2,#16

    LD2 {v4.2s, v5.2s}, [x8]
  ADD x8, x8,#16
    MOV  v6.16b, v5.16b
  movi  v5.2s, #0x00000000
    movi  v7.2s, #0x00000000

    LD1 {v5.s}[0],[x8],#4
    LD1 {v7.s}[0],[x8]

    MOV             x12, #16
  MOV        v4.d[1], v5.d[0]
  MOV        v6.d[1], v7.d[0]
//    VUZP.16         D4, D5

  UZP1      v19.8h, v4.8h, v4.8h
  UZP2      v21.8h, v4.8h, v4.8h
  MOV        v4.d[0], v19.d[0]
  MOV        v5.d[0], v21.d[0]

//    VUZP.16         D6, D7

  UZP1      v19.8h, v6.8h, v6.8h
  UZP2      v21.8h, v6.8h, v6.8h
  MOV        v6.d[0], v19.d[0]
  MOV        v7.d[0], v21.d[0]

    ADD             x6, x6, #16

    MOV             x12, #-4
    LD2 {v11.2s, v12.2s}, [x6]
  ADD x6, x6, x12
  MOV v13.16b, v12.16b


    movi  v10.2s, #0x00000000

    LD1 {v12.s}[1],[x6]
  ADD  x6, x6, x12
    LD1 {v10.s}[1],[x6]
    ADD  x6, x6, x12
  LD1 {v12.s}[0],[x6]
    ADD  x6, x6, x12

  MOV        v10.d[1], v11.d[0]
  MOV        v12.d[1], v13.d[0]

    //VUZP.16         D10, D11

  UZP1      v19.8h, v10.8h, v10.8h
  UZP2      v21.8h, v10.8h, v10.8h
  MOV        v10.d[0], v19.d[0]
  MOV        v10.d[1], v21.d[0]

  //VUZP.16         D12, D13

  UZP1      v19.8h, v12.8h, v12.8h
  UZP2      v21.8h, v12.8h, v12.8h
  MOV        v12.d[0], v19.d[0]
  MOV        v12.d[1], v21.d[0]


    rev64  v10.8h,  v10.8h
  MOV        v11.d[0], v10.d[1]
    rev64  v12.8h,  v12.8h
  MOV        v13.d[0], v12.d[1]

    uMULL v30.4s, v0.4h, v9.4h
    uMULL v28.4s, v2.4h, v8.4h
    uMULL v26.4s, v0.4h, v8.4h
    uMULL v24.4s, v2.4h, v9.4h

    ushR v30.4s, v30.4s,#16
    ushR v28.4s, v28.4s,#16

    sMLAL v30.4s, v1.4h, v9.4h
    sMLAL v28.4s, v3.4h, v8.4h

    ushR v26.4s, v26.4s,#16
    ushR v24.4s, v24.4s,#16

    sMLAL v26.4s, v1.4h, v8.4h
    sMLAL v24.4s, v3.4h, v9.4h

    ADD  v30.4s,  v30.4s ,  v28.4s
    NEG  v30.4s, v30.4s

    uMULL v22.4s, v4.4h, v8.4h

    SUB  v28.4s,  v24.4s ,  v26.4s


    mov  v26.16b, v30.16b
    mov  v24.16b, v28.16b

//    VUZP.16         D26, D27

  UZP1      v19.8h, v26.8h, v26.8h
  UZP2      v21.8h, v26.8h, v26.8h
  MOV        v26.d[0], v19.d[0]
  MOV        v27.d[0], v21.d[0]

    //VUZP.16         D24, D25

  UZP1      v19.8h, v24.8h, v24.8h
  UZP2      v21.8h, v24.8h, v24.8h
  MOV        v24.d[0], v19.d[0]
  MOV        v25.d[0], v21.d[0]

    uMULL v2.4s, v24.4h, v18.4h
    uMULL v0.4s, v26.4h, v18.4h

    ushR v22.4s, v22.4s,#16
    sMLAL v22.4s, v5.4h, v8.4h

    ushR v2.4s, v2.4s,#16
    ushR v0.4s, v0.4s,#16
    sMLAL v2.4s, v25.4h, v18.4h
    sMLAL v0.4s, v27.4h, v18.4h

    uMULL v24.4s, v4.4h, v9.4h
    uMULL v26.4s, v6.4h, v8.4h

    NEG  v2.4s, v2.4s
    ADD  v28.4s,  v28.4s ,  v0.4s
    ADD  v30.4s,  v30.4s ,  v2.4s

    uMULL v0.4s, v6.4h, v9.4h
    sshR v24.4s, v24.4s,#16
    sMLAL v24.4s, v5.4h, v9.4h
    sshR v26.4s, v26.4s,#16
    sshR v0.4s, v0.4s,#16
    sMLAL v26.4s, v7.4h, v8.4h
    sMLAL v0.4s, v7.4h, v9.4h

    ADD  v22.4s,  v22.4s ,  v0.4s
    NEG  v22.4s, v22.4s
    SUB  v24.4s,  v26.4s ,  v24.4s

    //LDR w11,  [sp, #120]
  //sxtw x11,w11
  MOV      w11, w26
    dup  v14.4s,w11
    SQADD  v28.4s,  v28.4s ,  v14.4s
    //LDR w11,  [sp, #116]
  //sxtw x11,w11
    MOV    w11, w25
  dup  v0.4s,w11
    sQshL v28.4s, v28.4s, v0.4s

    mov  v0.16b, v22.16b
    mov  v14.16b, v24.16b

//    VUZP.16         D22, D23

   UZP1      v19.8h, v22.8h, v22.8h
  UZP2      v21.8h, v22.8h, v22.8h
  MOV        v22.d[0], v19.d[0]
  MOV        v23.d[0], v21.d[0]

 //   VUZP.16         D24, D25

   UZP1      v19.8h, v24.8h, v24.8h
  UZP2      v21.8h, v24.8h, v24.8h
  MOV        v24.d[0], v19.d[0]
  MOV        v25.d[0], v21.d[0]

    uMULL v8.4s, v24.4h, v18.4h
    uMULL v26.4s, v22.4h, v18.4h

    NEG  v2.4s, v30.4s
//    VUZP.16         D30, D31

  UZP1      v19.8h, v30.8h, v30.8h
  UZP2      v21.8h, v30.8h, v30.8h
  MOV        v30.d[0], v19.d[0]
  MOV        v30.d[1], v21.d[0]

//    VUZP.16         D2, D3

  UZP1      v19.8h, v2.8h, v2.8h
  UZP2      v21.8h, v2.8h, v2.8h
  MOV        v2.d[0], v19.d[0]
  MOV        v3.d[0], v21.d[0]

    uMULL v4.4s, v30.4h, v12.4h
    uMULL v6.4s, v2.4h, v13.4h

    ushR v8.4s, v8.4s,#16
    ushR v26.4s, v26.4s,#16

    sMLAL v8.4s, v25.4h, v18.4h
    sMLAL v26.4s, v23.4h, v18.4h

    ushR v4.4s, v4.4s,#16
    ushR v6.4s, v6.4s,#16

  MOV  v19.d[0], v30.d[1]

    sMLAL v4.4s, v19.4h, v12.4h
    sMLAL v6.4s, v3.4h, v13.4h

    NEG  v8.4s, v8.4s
    ADD  v14.4s,  v14.4s ,  v26.4s
    ADD  v0.4s,  v0.4s ,  v8.4s

    //LDR w11,  [sp, #120]
  //sxtw x11,w11
  MOV      w11, w26
    dup  v8.4s,w11
    SQADD  v0.4s,  v0.4s ,  v8.4s
    //LDR w11,  [sp, #116]
  //sxtw x11,w11
    MOV    w11, w25
  dup  v26.4s,w11
    sQshL v0.4s, v0.4s, v26.4s

    mov  v26.16b, v28.16b

    MOV             x6, x4

    LD1 {v28.2s, v29.2s}, [x4],#16
    movi  v19.2s, #0x00000000
    LD1 {v30.s}[0],[x4],#4
    LD1 {v30.s}[1],[x4],#4
    LD1 {v19.s}[0],[x4],#4

  MOV        v28.d[1], v29.d[0]
  MOV        v30.d[1], v19.d[0]

    //VUZP.32         Q14, Q15

  UZP1      v19.4s, v28.4s, v30.4s
  UZP2      v30.4s, v28.4s, v30.4s
  MOV        v28.16b, v19.16b
  MOV        v29.d[0], v28.d[1]

    ST1 {v26.s}[0],[x6],#4
    ST1 {v0.s}[0],[x6],#4
    ST1 {v26.s}[1],[x6],#4
    ST1 {v0.s}[1],[x6],#4
    ST1 {v26.s}[2],[x6],#4
    ST1 {v0.s}[2],[x6],#4
    ST1 {v26.s}[3],[x6],#4

    movi  v1.2s, #0
    //VADDL.S16       Q0, D13, D1
  SADDL       v0.4s, v13.4h, v1.4h
  MOV      v1.d[0], v0.d[1]

    sMULL v26.2d, v28.2s, v0.2s
    Sqxtn v8.2s,  v26.2d
    sMULL v26.2d, v29.2s, v1.2s
    Sqxtn v9.2s,  v26.2d
  MOV    v8.d[1], v9.d[0]
    movi  v1.2s, #0
    //VADDL.S16       Q0, D12, D1
  SADDL       v0.4s, v12.4h, v1.4h
  MOV      v1.d[0], v0.d[1]

    sMULL v24.2d, v28.2s, v0.2s
    Sqxtn v26.2s,  v24.2d
    sMULL v24.2d, v29.2s, v1.2s
    Sqxtn v27.2s,  v24.2d
  MOV    v26.d[1], v27.d[0]

    sQshL v4.4s, v4.4s, v16.4s
    sQshL v6.4s, v6.4s, v16.4s

    SQSUB  v4.4s,  v4.4s ,  v8.4s
    SQSUB  v6.4s,  v6.4s ,  v26.4s

    NEG  v26.4s, v14.4s
    //VUZP.16         D14, D15

     UZP1      v19.8h, v14.8h, v14.8h
  UZP2      v21.8h, v14.8h, v14.8h
  MOV        v14.d[0], v19.d[0]
  MOV        v15.d[0], v21.d[0]

//  VUZP.16         D26, D27

  UZP1      v19.8h, v26.8h, v26.8h
  UZP2      v21.8h, v26.8h, v26.8h
  MOV        v26.d[0], v19.d[0]
  MOV        v27.d[0], v21.d[0]


    movi  v1.2s, #0
    //VADDL.S16       Q0, D10, D1
  SADDL       v0.4s, v10.4h, v1.4h
  MOV      v1.d[0], v0.d[1]

    sMULL v22.2d, v30.2s, v0.2s
    Sqxtn v24.2s,  v22.2d
    sMULL2 v22.2d, v30.4s, v0.4s
    Sqxtn v25.2s,  v22.2d
  MOV    v24.d[1], v25.d[0]

    movi  v1.2s, #0
//    VADDL.S16       Q0, D11, D1
  SADDL       v0.4s, v11.4h, v1.4h
  MOV      v1.d[0], v0.d[1]

    sMULL v8.2d, v30.2s, v0.2s
    Sqxtn v22.2s,  v8.2d
    sMULL2 v8.2d, v30.4s, v0.4s
    Sqxtn v23.2s,  v8.2d
  MOV    v22.d[1], v23.d[0]

    uMULL v8.4s, v26.4h, v11.4h
    uMULL v30.4s, v14.4h, v10.4h

    ushR v8.4s, v8.4s,#16

    ushR v30.4s, v30.4s,#16

    sMLAL v8.4s, v27.4h, v11.4h

    sMLAL v30.4s, v15.4h, v10.4h

    MOV V14.16B , V4.16B

    //mov  v15.8b, v6.8b
    sQshL v8.4s, v8.4s, v16.4s

    sQshL v30.4s, v30.4s, v16.4s

    SQSUB  v8.4s,  v8.4s ,  v24.4s

    SQSUB  v22.4s,  v30.4s ,  v22.4s

     MOV  V30.16B, V8.16B







    ST1 {V14.S}[0],[x0]
  ADD  x0, x0, x9
    ST1 {V22.S}[0],[x0]
  ADD  x0, x0, x9
    ST1 {V14.S}[1],[x0]
  ADD  x0, x0, x9
    ST1 {V22.S}[1],[x0]
  ADD  x0, x0, x9
    ST1 {V14.S}[2],[x0]
  ADD  x0, x0, x9
    ST1 {V22.S}[2],[x0]
  ADD  x0, x0, x9
    ST1 {V14.S}[3],[x0]
  ADD  x0, x0, x9

    ST1 {V6.S}[0],[x5]
  ADD  x5, x5, x10
    ST1 {V30.S}[0],[x5]
  ADD  x5, x5, x10
    ST1 {V6.S}[1],[x5]
  ADD  x5, x5, x10
    ST1 {V30.S}[1],[x5]
  ADD  x5, x5, x10
    ST1 {V6.S}[2],[x5]
  ADD  x5, x5, x10
    ST1 {V30.S}[2],[x5]
  ADD  x5, x5, x10
    ST1 {V6.S}[3],[x5]
  ADD  x5, x5, x10

   pop_v_regs
   ret


