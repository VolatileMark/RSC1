# RC1 ISA

## System properties

The system stores values in the **big endian format**.

| Registers | Width   | Usage              | Access |
| --------- | ------- | ------------------ | ------ |
| r0 - r7   | 16 bits | General purpose    | RW     |
| c0 - c1   | 16 bits | Control registers  | RW     |
| sp        | 16 bits | Stack pointer      | RW     |
| fg        | 16 bits | Flags register     | **     |
| pc        | 16 bits | Program counter    | R-     |

**NOTE**: `fg` can only be read and written to with the instructions `PUSHR` and `POPR`

| Instruction | Arguments | Description                                                     |
| ----------- | --------- | --------------------------------------------------------------- |
| AND         | rX, rY    | Bitwise and rX with rY                                          | 
| NOT         | rX        | Bitwise not rX                                                  |
| ADD         | rX, rY    | Add content of rY to rX                                         |
| MOV         | rX, rY    | Copy the value of rY into rX                                    |
| LDI         | rX, ##    | Load immediate 8-bit value ## into the lower 8-bits of rX       |
| LDB (LDA)   | rX, rY    | Load byte from memory address (rY) into rX                      |
| LDW (LDA)   | rX, rY    | Load word from memory address (rY) into rX                      |
| STB (STA)   | rY, rX    | Store the value of rX into memory at address (rY)               |
| STW (STA)   | rY, rX    | Store the value of rX into memory at address (rY)               |
| JMP         | rX        | Start executing instructions from address (rX)                  |
| JNZ         | rX, rY    | Jump to address (rX) only if rY is not zero                     |
| PUSHR       | rX        | Decrement sp and store the value of rX at memory address (sp)   |
| PUSHF       | --        | Decrement sp and store the value of fg at memory address (sp)   |
| POPR        | rX        | Store the value at memory address (sp) into rX and increment sp |
| POPF        | --        | Store the value at memory address (sp) into fg and increment sp |
| SHR         | rX, ##    | Bit shift the value in rX to the right by ## (8-bit) bits       |
| SHL         | rX, ##    | Bit shift the value in rX to the left by ## (8-bit) bits        |
| CALL        | rX        | PUSHR the address of the next function and jump to rX           |
| RET         | --        | POPR rX and JMP rX                                              |
| HLT         | --        | Halt execution until the next interrupt fires                   |
