#include <mutex>


template<typename T> class MTVariable
{
private:
	T instance;
	std::mutex mutex;

public:
	MTVariable() : instance(empty()) { }
	MTVariable(T instance) : instance(instance) { }

	//Returns default instance
	T empty()
	{
		return T();
	}

	T get()
	{
		auto lock = std::lock_guard<std::mutex>(mutex);
		//Return copy
		return T(instance);
	}

	void set(T value)
	{
		auto lock = std::lock_guard<std::mutex>(mutex);
		instance = value;
	}

	void operator=(MTVariable<T> other)
	{
		set(other.instance);
	}
};
