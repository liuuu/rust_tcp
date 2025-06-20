i wanna implement a server

- in this server there will be two client will connected, when they send "DATA", the server will send some radar data to the client,
- the two client use Double Buffering Cache Processing Mechanism to receive data, merge the data using **Sliding Window Algorithm** then send data to another process to do mathmatic like abs and log , then draw the image with png format, the color bar value can be configured
- implement this server
- contruct the radar data just for development thus the two client can do merge

- just as much as possible to mimic the realworld, how the radar data will be send to the client, how the data will be merged, i think the data is send to client continiously,
  what mechanism the two client can merge the data
- just think throughly to implement the server side, i will implement client in another porject
- leave the color bar setting for later iteration
- i care the most how to server generate data like realworld data, and how the data can be tansfered adn merged in the client to draw a png picture

# Critical Questions for Implementation

1. Overlap Strategy: How much spatial overlap between clients? (e.g., 10% of azimuth range?)
   anwser: u dicide it, 10% ok?

2. Data Rate: How frequently should frames be sent? (e.g., 10 Hz, 20 Hz?)
   anwser: for a simple working server, 5Hz ok?

3. Radar Parameters:

- Azimuth coverage per client? (e.g., Client 1: 0-180°, Client 2: 170-360°)
  anwser: if is ok for merge and generate png
- Range resolution and maximum range?
  anwser: you can decide it
- Number of range bins?
  anwser: you dicide

4. Synchronization: Should clients wait for both datasets before merging?
   anwser: the client use double buffering caching for receive data, when all the front buffer full, start the merging

5. Target Simulation: What types of targets to simulate? (aircraft, weather, ground clutter?)
   anwser: any simulation that let the client can merge and generate png

Real-World Scenario
In reality, ONE radar system generates ONE complete sweep, but this data needs to be:

1. Split into chunks for processing efficiency
2. Distributed to multiple processing nodes
3. Merged back to create the complete picture

[Radar Antenna] → [Raw Data] → [Data Splitter] → [Client 1: Az 0-180°]
                                              → [Client 2: Az 180-360°]
                                              
Both clients receive data from THE SAME radar sweep, just different spatial regions
