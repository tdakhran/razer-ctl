import pandas as pd
import numpy as np
from tabulate import tabulate

raw_data = pd.read_csv('data/wireshark_dump_raw.csv', sep='\t', header=None)
annotations = pd.read_csv('data/annotations.csv', sep=' ', header=None)

table = [['action', 'cmd', 'argc', 'arg0', 'arg1', 'arg2', 'arg3']]

for (index, annotation) in annotations.iterrows():
    time_s = annotation[0]
    description = annotation[1]
    usb_frames = (raw_data[raw_data[1].astype(int) == time_s])[6]
    row = [description]
    for frame in usb_frames:
        argc = int(frame[10:12], 16)
        cmd = frame[12:16]
        row += [cmd, argc]
        for i in range(argc):
            row += [frame[(16 + 2 * i):(16 + 2 * i + 2)]]
        table.append(row)
        row = ['']

print(tabulate(table, headers='firstrow', tablefmt='github'))
