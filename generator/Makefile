
.PHONY: clean

run: ./build/main
	@./build/main ../data/mini.concat ../data/mini.bwt ../data/mini.lcp

build: ./build/main

./build/main: ./main.c ./build/libsais.o ./build/libsais64.o
	gcc -Wall -Wextra -I../../libsais/include -march=native -O3 main.c ./build/libsais64.o ./build/libsais.o -o ./build/main

./build/libsais64.o:
	mkdir -p build
	gcc -I../../libsais/include -march=native -O3 ../../libsais/src/libsais64.c -c -o ./build/libsais64.o

./build/libsais.o:
	mkdir -p build
	gcc -I../../libsais/include -march=native -O3 ../../libsais/src/libsais.c -c -o ./build/libsais.o

clean:
	rm -f ./build/libsais64.o ./build/libsais.o ./build/main

