with open("aitrans_block.txt", 'w') as f:
    for i in range(10):
        f.write('%f %d %d %d\n' % (0.02 * i, 9999999 , 1000000, 1))
