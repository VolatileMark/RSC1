# RSC1 Instruction Set

## System properties

The system stores values in the **little endian format**.

| Registers | Width   | Usage              | Access |
| --------- | ------- | ------------------ | ------ |
| r0 - r7   | 16 bits | General purpose    | RW     |
| sp        | 16 bits | Stack pointer      | RW     |
| c0 - c1   | 16 bits | Control registers  | RW     |
| fg        | 16 bits | Flags register     | **     |
| pc        | 16 bits | Program counter    | R-     |

## Instructions

| Opcode | Instruction | Arguments | Valid Values | Description                                                     |
| ------ | ----------- | --------- | ------------ | --------------------------------------------------------------- |
| 0x0000 | NOP         | --, --    |    --, --    | Do nothing for 1 machine cycle                                  |
| 0x1XY0 | AND         |  X, Y     | r0-r7, r0-r7 | Bitwise and X with Y                                            |
| 0x1X01 | NOT         |  X, --    | r0-r7, --    | Bitwise not X                                                   |
| 0x2XY0 | ADD         |  X, Y     | r0-r7, r0-r7 | Add content of Y to X                                           |
| 0x2XY1 | SUB         |  X, Y     | r0-r7, r0-r7 | Subtract content of Y from X                                    |
| 0x2X02 | INC         |  X, --    | r0-sp, --    | Increment the value of X by 1                                   |
| 0x2X03 | DEC         |  X, --    | r0-sp, --    | Decrement the value of X by 1                                   |
| 0x3XY0 | LDB         |  X, Y     | r0-r7, r0-sp | Load byte from memory address (Y) into X                        |
| 0x3XY1 | LDW         |  X, Y     | r0-r7, r0-sp | Load word from memory address (Y) into X                        |
| 0x3XY2 | MOV         |  X, Y     | r0-c1, r0-c1 | Copy the value of Y into X                                      |
| 0x4XNN | LDI         |  X, NN    | r0-r7, 0-255 | Load immediate 8-bit value ## into the lower 8-bits of X        |
| 0x5YX0 | STB         |  Y, X     | r0-sp, r0-r7 | Store the value of X into memory at address (Y)                 |
| 0x5YX1 | STW         |  Y, X     | r0-sp, r0-r7 | Store the value of X into memory at address (Y)                 |
| 0x6X00 | JMP         |  X, --    | r0-sp, --    | Start executing instructions from address (X)                   |
| 0x6XY1 | JNZ         |  X, Y     | r0-sp, r0-r7 | Jump to address (X) only if Y is not zero                       |
| 0x7XN0 | SHR         |  X, N     | r0-r7, 0-15  | Bit shift the value in X to the right by N (4-bit) bits         |
| 0x7XN1 | SHL         |  X, N     | r0-r7, 0-15  | Bit shift the value in X to the left by N (4-bit) bits          |
| 0x8N00 | TEST        |  N, --    |  0-15, --    | Skip next instruction if bit N in fg is set                     |
| 0x8N01 | SETF        |  N, --    |  0-15, --    | Set bit N of fg register                                        |
| 0x8N02 | CLRF        |  N, --    |  0-15, --    | Clear bit N of fg register                                      |

## Assembler Pseudo-Instructions

| Pseudo-Instruction | Arguments | Valid Values | Equivalent    |
| ------------------ | --------- | ------------ | ------------- |
| PUSH               | X         | r0-r7, --    | DEC sp        |
|                    |           |              | DEC sp        |
|                    |           |              | STW sp, X     |
| POP                | X         | r0-r7, --    | LDW  X, sp    |
|                    |           |              | INC  sp       |
|                    |           |              | INC  sp       |
| LDL                | X, AABB   | r0-r7, addr  | LDI  X, AA    |
|                    |           |              | SHL  X, 8     |
|                    |           |              | LDI  X, BB    |
| CALL               | X         | r0-r7, --    | DEC sp        |
|                    |           |              | DEC sp        |
|                    |           |              | DEC sp        |
|                    |           |              | DEC sp        |
|                    |           |              | STW sp, X     |
|                    |           |              | INC sp        |
|                    |           |              | INC sp        |
|                    |           |              | LDL  X, $+16  |
|                    |           |              | STW sp, X     |
|                    |           |              | DEC sp        |
|                    |           |              | DEC sp        |
|                    |           |              | LDW  X, sp    |
|                    |           |              | INC sp        |
|                    |           |              | INC sp        |
|                    |           |              | JMP  X        |
| CALLF              | X, Y      | r0-sp, r0-r7 | LDL  Y, $+6   |
|                    |           |              | PUSH Y        |
|                    |           |              | JMP  X        |
| RET                | X, --     | r0-r7, --    | LDW  X, sp    |
|                    |           |              | INC sp        |
|                    |           |              | INC sp        |
|                    |           |              | JMP  X        |

## Assembler Directives

| Directive | Arguments | Description                                 |
| --------- | --------- | ------------------------------------------- |
| .short    | val/label | Write a 16-bit value at the current address |
| .addr     | addr      | Set the executable address                  |
