jar cvf /tmp/ringbridge.jar -C /tmp/rh/classes .
mvn install:install-file -Dfile=/tmp/ringbridge.jar -DgroupId=io.privacyresearch -DartifactId=ringbridge -Dversion=0.0.1-SNAPSHOT -Dpackaging=jar

