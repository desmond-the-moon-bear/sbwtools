
run: ./build/main
	@./build/main

./build/main: ./main.c ./build/libsais.o
	gcc -I../../libsais/include -march=native -O3 main.c ./build/libsais.o -o ./build/main

./build/libsais.o:
	gcc -I../../libsais/include -march=native -O3 ../../libsais/src/libsais.c -c -o ./build/libsais.o

clean:
	rm -f ./build/libsais.o ./build/main

