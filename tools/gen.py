for seed in range(0, 101):
    # in/{seed04}.txtを作成しseed*seedを出力
    with open(f'tools/in/{seed:04}.txt', 'w') as f:
        print(seed*seed, file=f)