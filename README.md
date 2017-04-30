Iceflux
===============================

Import statistics from [Icecast2](https://icecast.org) to [InfluxDB](https://www.influxdata.com/influxdb/).

## Contents
1. [Development Status](#development-status)
2. [Getting Started](#getting-started)
    1. [With Docker](#with-docker)
       1. [Get InfluxDB](#get-influxdb)
       2. [Run Iceflux](#run-iceflux)
    2. [Without Docker](#without-docker)

## Development Status
This project is currently something of a prototype. It's written in Rust, a language I'm slowly learning, so the structure of the program is fairly terrible.

It is, however, functional, and is in use by [St Andrews Radio](https://www.standrewsradio.com), at least by the Tech Team there to track Listener numbers on the various streams offered across the suite of StAR applications.

A vague roadmap for future development includes increasing the number of Icecast statistics reported, including possibly the `/admin/stats` endpoint and the Icecast log files, as well as general restructuring of the code. It would be nice if this restructuring switched to using some nice Futures, and something a bit better than the current loop-and-sleep mechanism to rerun the stats gathering.

## Getting Started
### With Docker
#### Get InfluxDB
If you already have InfluxDB up and running, you can skip this section.
1. Get the InfluxDB container and start it:

        docker run -d -p 8086:8086 \
            -v influxdb:/var/lib/influxdb \
            --name=influxdb \
            influxdb
2. Create the Icecast database and users:

        docker run --rm --link=influxdb -it influxdb influx -host influxdb
        CREATE DATABASE icecast;
        CREATE USER icecast WITH PASSWORD icecast
        GRANT ALL ON icecast TO icecast

#### Run Iceflux
1. Clone the project: `git clone git@github.com:angusi/iceflux.git`
2. Build the project: `cargo build --release`
3. Build the Docker container: `docker build -t angusi/iceflux .`
3. Configure the application's environment variables:  

        cp example.env production.env
        $EDITOR production.env
    
4. Start the Iceflux container: 

        docker run -d --env-file=production.env \
            --name=iceflux \
            angusi/iceflux
        
### Without Docker
You'll need to have set up InfluxDB separately first. Try the [InfluxDB Docs](https://docs.influxdata.com/influxdb/). Then...

1. Clone the project: `git clone git@github.com:angusi/iceflux.git`
2. Build the project: `cargo build --release`
3. Configure the application through environment variables: 

        export ICECAST_USER=admin
        export ICECAST_PASSWORD=icecast
        export ICECAST_HOST=icecast.example.com
        export ICECAST_PORT=8080
        
        export INFLUX_USER=icecast
        export INFLUX_PASSWORD=icecast
        export INFLUX_HOST=localhost
        export INFLUX_PORT=8086
        export INFLUX_DATABASE=icecast

4. Start the application: `./target/release/iceflux`

