set(CMAKE_SYSTEM_NAME Linux)
set(CMAKE_SYSTEM_PROCESSOR aarch32)

# Modify these variables with paths to appropriate compilers that can produce
# armv7 targets

# Specify the cross compiler
set(CMAKE_C_COMPILER arm-none-linux-gnueabi-gcc)
set(CMAKE_CXX_COMPILER arm-none-linux-gnueabi-g++)
set(CMAKE_C_COMPILER_AR arm-none-linux-gnueabi-ar)
set(CMAKE_CXX_COMPILER_AR arm-none-linux-gnueabi-ar)
