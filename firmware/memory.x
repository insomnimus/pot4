MEMORY
{
    FLASH : ORIGIN = 0x08000000, LENGTH = 254K /* BANK_1 minus the final page */
    RAM   : ORIGIN = 0x20000000, LENGTH =   40K /* SRAM */
}