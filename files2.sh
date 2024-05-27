#!/bin/bash
rm -rf big_directory
mkdir big_directory
for ((i=0; i<10000; i++)); do
	touch "big_directory/fichero_de_texto_vacio_pero_con_nombre_largo{$i}.txt"
done
