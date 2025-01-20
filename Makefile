all: comp

comp: prj2
	docker compose docker-compose.yml up
prj2:
	docker build . -t $@
