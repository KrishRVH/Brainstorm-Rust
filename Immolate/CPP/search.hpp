#ifndef SEARCH_HPP
#define SEARCH_HPP

#include "instance.hpp"
#include <algorithm>
#include <atomic>
#include <functional>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

class Search {
   public:
    static constexpr long long kSeedSpace = 2318107019761LL;
    static constexpr long long kBlockSize = 1000000LL;

    std::atomic<long long> highScore{1};
    std::function<int(Instance&)> filter;
    std::atomic<bool> found{false};  // Atomic flag to signal when a solution is found
    Seed foundSeed;                  // Store the found seed
    bool exitOnFind = false;
    long long startSeed;
    int numThreads;
    long long numSeeds;
    std::mutex mtx;
    std::atomic<long long> nextBlock{0};  // Shared index for the next block to be processed

    explicit Search(std::function<int(Instance&)> f) : Search(std::move(f), 1, kSeedSpace, 0) {}

    Search(std::function<int(Instance&)> f, int t) : Search(std::move(f), t, kSeedSpace, 0) {}

    Search(std::function<int(Instance&)> f, int t, long long n) : Search(std::move(f), t, n, 0) {}

    Search(std::function<int(Instance&)> f, const std::string& seed, int t, long long n)
        : Search(std::move(f), t, n, Seed(seed).getID()) {}

    Search(std::function<int(Instance&)> f, int t, long long n, long long start)
        : filter(std::move(f)), startSeed(start), numThreads(t), numSeeds(n) {}

    void searchBlock(long long start, long long end) {
        Seed s(start);
        Instance inst(s);
        for (long long i = start; i < end; ++i) {
            if (found.load(std::memory_order_relaxed))
                return;  // Exit if a solution is found
            // Perform the search on the seed
            int result = filter(inst);
            if (result >= highScore.load(std::memory_order_relaxed)) {
                std::lock_guard<std::mutex> lock(mtx);
                if (result >= highScore.load(std::memory_order_relaxed)) {
                    highScore.store(result, std::memory_order_relaxed);
                    foundSeed = s;
                    if (exitOnFind) {
                        found.store(true, std::memory_order_relaxed);
                        return;
                    }
                }
            }
            inst.next();
        }
    }

    std::string search() {
        std::vector<std::thread> threads;
        threads.reserve(numThreads);
        long long totalBlocks = (numSeeds + kBlockSize - 1) / kBlockSize;
        for (int t = 0; t < numThreads; t++) {
            threads.emplace_back([this, totalBlocks]() {
                while (true) {
                    if (found.load(std::memory_order_relaxed))
                        break;
                    long long block = nextBlock.fetch_add(1, std::memory_order_relaxed);
                    if (block >= totalBlocks)
                        break;
                    long long start = block * kBlockSize + startSeed;
                    long long end = std::min(start + kBlockSize, numSeeds + startSeed);
                    searchBlock(start, end);
                }
            });
        }

        for (auto& thread : threads) {
            thread.join();
        }

        return foundSeed.tostring();
    }
};

#endif
