for i in $(docker ps -aq); do
    docker rm $i
done

for i in $(docker images -aq); do
    docker rmi $i
done
